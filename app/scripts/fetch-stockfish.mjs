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
import { execSync } from "node:child_process";
import { createWriteStream, existsSync, mkdirSync, readdirSync, renameSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { exec as execCb } from "node:child_process";
import { promisify } from "node:util";
const exec = promisify(execCb);

const BINARIES_DIR = resolve(process.cwd(), "src-tauri/binaries");
const SF_TAG = "sf_18";

/** Map our known targets to a Stockfish release asset. */
function assetFor(target) {
  const isWin = target.includes("windows");
  const isMac = target.includes("apple-darwin");
  const isLinux = target.includes("linux");
  const arch = target.includes("aarch64") || target.includes("arm64") ? "arm" : "x86-64";

  // Prefer broadly-compatible variants (no AVX2 requirement) for portability.
  if (isWin) {
    return arch === "arm" ? "stockfish-windows-armv8.zip" : "stockfish-windows-x86-64.zip";
  }
  if (isMac) {
    return arch === "arm"
      ? "stockfish-macos-m1-apple-silicon.tar"
      : "stockfish-macos-x86-64.tar";
  }
  if (isLinux) {
    return arch === "arm" ? "stockfish-android-armv8.tar" : "stockfish-ubuntu-x86-64.tar";
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
function findBinary(dir, ext) {
  const entries = readdirSync(dir, { recursive: true });
  for (const entry of entries) {
    const full = join(dir, String(entry));
    try {
      if (!statSync(full).isFile()) continue;
    } catch {
      continue;
    }
    const name = String(entry);
    if (!/^stockfish/i.test(name)) continue;
    if (ext ? name.endsWith(ext) : !name.endsWith(".exe")) return full;
  }
  return null;
}

async function main() {
  const target = hostTuple();
  if (!target) throw new Error("Não consegui determinar o target triple do Rust.");
  const ext = target.includes("windows") ? ".exe" : "";
  const finalPath = join(BINARIES_DIR, `stockfish-${target}${ext}`);
  if (existsSync(finalPath)) {
    console.log("✓ Já existe:", finalPath);
    return;
  }

  const asset = assetFor(target);
  const url = `https://github.com/official-stockfish/Stockfish/releases/download/${SF_TAG}/${asset}`;
  mkdirSync(BINARIES_DIR, { recursive: true });

  const tmp = join(tmpdir(), `sf-${Date.now()}`);
  mkdirSync(tmp, { recursive: true });
  const archivePath = join(tmp, asset);
  await download(url, archivePath);

  const extractDir = join(tmp, "out");
  mkdirSync(extractDir, { recursive: true });
  if (asset.endsWith(".tar")) await extractTar(archivePath, extractDir);
  else await extractZip(archivePath, extractDir);

  const bin = findBinary(extractDir, ext);
  if (!bin) throw new Error("Binário do Stockfish não encontrado no pacote extraído.");

  if (!ext) await exec(`chmod +x "${bin}"`);
  renameSync(bin, finalPath);
  rmSync(tmp, { recursive: true, force: true });

  console.log("✓ Stockfish instalado em:", finalPath);
}

main().catch((e) => {
  console.error("✗", e?.message || e);
  process.exit(1);
});
