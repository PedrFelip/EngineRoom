#!/usr/bin/env node
/**
 * Downloads and verifies Stockfish for the current target.
 * Run with node scripts/fetch-stockfish.mjs.
 */
import { execFile, execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, createWriteStream, existsSync, mkdirSync, readdirSync, renameSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { promisify } from "node:util";
import { fileURLToPath, pathToFileURL } from "node:url";
const execFileAsync = promisify(execFile);

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
    return execFileSync("rustc", ["--print", "host-tuple"]).toString().trim();
  } catch {
    const v = execFileSync("rustc", ["-Vv"]).toString();
    return (v.match(/host:\s*(\S+)/) || [])[1];
  }
}

async function download(url, dest) {
  console.log("⬇", url);
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok || !res.body) throw new Error(`HTTP ${res.status} para ${url}`);
  await pipeline(res.body, createWriteStream(dest));
}

export async function extractTar(archivePath, outDir) {
  await execFileAsync("tar", ["-xf", archivePath, "-C", outDir]);
}

async function expandArchiveWithPowerShell(archivePath, outDir) {
  // Pass paths through the environment so PowerShell never parses them as code.
  await execFileAsync(
    "powershell",
    [
      "-NoLogo",
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      "Expand-Archive -LiteralPath $env:ENGINEROOM_ARCHIVE -DestinationPath $env:ENGINEROOM_OUT -Force",
    ],
    {
      env: {
        ...process.env,
        ENGINEROOM_ARCHIVE: archivePath,
        ENGINEROOM_OUT: outDir,
      },
    },
  );
}

export async function extractZip(archivePath, outDir) {
  // `unzip` is available on most systems (incl. Git Bash on Windows).
  try {
    await execFileAsync("unzip", ["-o", "-q", archivePath, "-d", outDir]);
  } catch {
    // Fallback to PowerShell Expand-Archive (Windows without unzip).
    await expandArchiveWithPowerShell(archivePath, outDir);
  }
}

/** Mark a staged sidecar executable without shell interpolation. */
export function markExecutable(path) {
  chmodSync(path, 0o755);
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

export async function main(binariesDir = new URL("../binaries/", import.meta.url)) {
  if (binariesDir instanceof URL) binariesDir = fileURLToPath(binariesDir);
  const target = hostTuple();
  if (!target) throw new Error("Não consegui determinar o target triple do Rust.");
  const ext = target.includes("windows") ? ".exe" : "";
  const asset = assetFor(target);
  const finalPath = join(binariesDir, `stockfish-${target}${ext}`);
  if (existsSync(finalPath) && hasExpectedVersion(finalPath)) {
    console.log("✓ Stockfish 19 já instalado:", finalPath);
    return;
  }

  const url = `https://github.com/official-stockfish/Stockfish/releases/download/${SF_TAG}/${asset}`;
  mkdirSync(binariesDir, { recursive: true });

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

    if (!ext) markExecutable(bin);
    if (!hasExpectedVersion(bin)) {
      throw new Error("O pacote baixado não identificou Stockfish 19 via UCI.");
    }
    copyFileSync(bin, stagedPath);
    if (!ext) markExecutable(stagedPath);
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
