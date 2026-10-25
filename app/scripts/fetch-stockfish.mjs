#!/usr/bin/env node
/**
 * Downloads the Stockfish binary for the current platform/target-triple into
 * `src-tauri/binaries/stockfish-<target-triple>[.exe]`, which is where Tauri
 * looks for the sidecar declared in tauri.conf.json (`bundle.externalBin`).
 *
 * Run it with:  node scripts/fetch-stockfish.mjs
 * (or `bun scripts/fetch-stockfish.mjs`)
 *
 * The repo does not commit the binaries (see src-tauri/.gitignore), so every
 * developer / CI run fetches the right one for their platform.
 */
import { execFileSync, execSync } from "node:child_process";
import { copyFileSync, createWriteStream, existsSync, mkdirSync, readdirSync, renameSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { exec as execCb } from "node:child_process";
import { promisify } from "node:util";
import { pathToFileURL } from "node:url";
const exec = promisify(execCb);

const BINARIES_DIR = resolve(process.cwd(), "src-tauri/binaries");
const SF_TAG = "sf_19";

/** Map our known targets to a Stockfish release asset. */
export function assetFor(target) {
  const isWin = target.includes("windows");
  const isMac = target.includes("apple-darwin");
  const isLinux = target.includes("linux");
  const arch = /^(aarch64|arm64)-/.test(target)
    ? "arm64"
    : target.startsWith("x86_64-") ? "x86-64" : null;
  if (!arch) throw new Error(`Arquitetura não suportada: ${target}`);

  // Universal binaries select the optimal instructions for the host CPU.
  if (isWin) {
    return `stockfish-windows-${arch}-universal.zip`;
  }
  if (isMac) {
    return "stockfish-macos-universal.tar.gz";
  }
  if (isLinux) {
    if (!target.endsWith("-gnu")) {
      throw new Error(`Target Linux não suportado: ${target}`);
    }
    return `stockfish-linux-${arch}-universal.tar.gz`;
  }
  throw new Error(`Plataforma não suportada pelo fetch script: ${target}`);
}

function hostTuple() {
  try {
    return execSync("rustc --print host-tuple").toString().trim();
  } catch {
    const v = execSync("rustc -Vv").toString();
    return (v.match(/host:\s*(\S+)/) || [])[1];
  }
}

async function download(url, dest) {
  console.log("⬇", url);
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok || !res.body) throw new Error(`HTTP ${res.status} para ${url}`);
  await pipeline(res.body, createWriteStream(dest));
}

async function extractTar(archivePath, outDir) {
  await exec(`tar -xf "${archivePath}" -C "${outDir}"`);
}

async function extractZip(archivePath, outDir) {
  // `unzip` is available on most systems (incl. Git Bash on Windows).
  try {
    await exec(`unzip -o -q "${archivePath}" -d "${outDir}"`);
  } catch {
    // Fallback to PowerShell Expand-Archive (Windows without unzip).
    await exec(`powershell -NoProfile -Command "Expand-Archive -LiteralPath '${archivePath}' -DestinationPath '${outDir}' -Force"`);
  }
}

/** Find the actual stockfish executable inside the extracted dir. */
function findBinary(dir, expectedName) {
  const entries = readdirSync(dir, { recursive: true });
  for (const entry of entries) {
    const full = join(dir, String(entry));
    try {
      if (!statSync(full).isFile()) continue;
    } catch {
      continue;
    }
    const name = basename(String(entry));
    if (name === expectedName) return full;
  }
  return null;
}

export function isExpectedVersion(output) {
  return /^id name Stockfish 19\s*$/m.test(output);
}

function hasExpectedVersion(binary) {
  try {
    return isExpectedVersion(execFileSync(binary, [], {
      input: "uci\nquit\n",
      encoding: "utf8",
      timeout: 15000,
      maxBuffer: 1024 * 1024,
      stdio: ["pipe", "pipe", "pipe"],
    }));
  } catch {
    return false;
  }
}

async function main() {
  const target = hostTuple();
  if (!target) throw new Error("Não consegui determinar o target triple do Rust.");
  const ext = target.includes("windows") ? ".exe" : "";
  const asset = assetFor(target);
  const finalPath = join(BINARIES_DIR, `stockfish-${target}${ext}`);
  if (existsSync(finalPath) && hasExpectedVersion(finalPath)) {
    console.log("✓ Stockfish 19 já instalado:", finalPath);
    return;
  }

  const url = `https://github.com/official-stockfish/Stockfish/releases/download/${SF_TAG}/${asset}`;
  mkdirSync(BINARIES_DIR, { recursive: true });

  const tmp = join(tmpdir(), `sf-${Date.now()}`);
  mkdirSync(tmp, { recursive: true });
  const archivePath = join(tmp, asset);
  const stagedPath = `${finalPath}.download`;
  try {
    await download(url, archivePath);

    const extractDir = join(tmp, "out");
    mkdirSync(extractDir, { recursive: true });
    if (asset.endsWith(".tar.gz")) await extractTar(archivePath, extractDir);
    else await extractZip(archivePath, extractDir);

    const expectedName = asset.replace(/\.(tar\.gz|zip)$/, "") + ext;
    const bin = findBinary(extractDir, expectedName);
    if (!bin) throw new Error("Binário do Stockfish não encontrado no pacote extraído.");

    if (!ext) await exec(`chmod +x "${bin}"`);
    if (!hasExpectedVersion(bin)) {
      throw new Error("O pacote baixado não identificou Stockfish 19 via UCI.");
    }
    copyFileSync(bin, stagedPath);
    if (!ext) await exec(`chmod +x "${stagedPath}"`);
    renameSync(stagedPath, finalPath);

    console.log("✓ Stockfish 19 instalado em:", finalPath);
  } finally {
    rmSync(stagedPath, { force: true });
    rmSync(tmp, { recursive: true, force: true });
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  main().catch((e) => {
    console.error("✗", e?.message || e);
    process.exit(1);
  });
}
