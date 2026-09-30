#!/usr/bin/env python3
"""Run sequential TS/Rust pipeline benchmarks and validate comparable work."""
import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys

APP = Path(__file__).resolve().parents[1]


def command(args, cwd=APP):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def normalized(value):
    if isinstance(value, dict):
        return {k: normalized(v) for k, v in value.items() if v is not None}
    if isinstance(value, list):
        return [normalized(v) for v in value]
    return value


def compare(a, b, path="result"):
    if isinstance(a, dict) and isinstance(b, dict):
        if a.keys() != b.keys():
            raise ValueError(f"{path}: different keys {a.keys() ^ b.keys()}")
        for key in a:
            compare(a[key], b[key], f"{path}.{key}")
    elif isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            raise ValueError(f"{path}: different lengths {len(a)} / {len(b)}")
        for i, (left, right) in enumerate(zip(a, b)):
            compare(left, right, f"{path}[{i}]")
    elif isinstance(a, (int, float)) and isinstance(b, (int, float)):
        if not math.isfinite(a) or not math.isfinite(b) or abs(a - b) > 1e-9:
            raise ValueError(f"{path}: {a} != {b}")
    elif a != b:
        raise ValueError(f"{path}: {a!r} != {b!r}")


def metrics(values):
    ordered = sorted(values)
    return {
        "medianMs": statistics.median(ordered),
        "p95Ms": ordered[math.ceil(len(ordered) * 0.95) - 1],
        "minMs": ordered[0], "maxMs": ordered[-1],
        "samples": len(ordered), "elapsedMs": values,
    }


def summarize(output, fixture, runs, metadata):
    rows = []
    failures = []
    for spec in fixture["scenarios"]:
        identifier = spec["id"]
        by_runtime = {
            runtime: [next(s for s in report["scenarios"] if s["id"] == identifier)
                      for report in runs[runtime]]
            for runtime in ("ts", "rust")
        }
        row = {"id": identifier, "spec": spec, "unit": "game" if spec["task"] == "game" else "search"}
        errors = [s["error"] for s in by_runtime["ts"] if "error" in s]
        if errors:
            # The frozen TS implementation never supported replaying a custom initial FEN.
            expected = spec["game"] in ("promotion", "blackFen")
            row.update(status="unsupported-ts" if expected else "error", error=errors[0])
            if not expected:
                failures.append(f"{identifier}: {errors[0]}")
        else:
            try:
                for ts, rust in zip(by_runtime["ts"], by_runtime["rust"]):
                    ts_stats, rust_stats = dict(ts["stats"]), dict(rust["stats"])
                    if spec.get("engine") == "stockfish":
                        # Time-budget searches are nondeterministic; info volume varies.
                        ts_stats.pop("infoLines")
                        rust_stats.pop("infoLines")
                        compare(ts_stats, rust_stats, "workload")
                        # Moves, PGN, positions and output structure must still agree.
                        for key in ("moves", "positions"):
                            compare([v["fen"] if key == "positions" else {k: v[k] for k in ("ply", "color", "san", "uci", "fenBefore")} for v in ts["result"][key]],
                                    [v["fen"] if key == "positions" else {k: v[k] for k in ("ply", "color", "san", "uci", "fenBefore")} for v in rust["result"][key]], key)
                        for result in (ts["result"], rust["result"]):
                            assert all(0 <= n <= 100 for n in result["accuracy"].values())
                        row["parity"] = "workload and mainline; scores not compared for real engine"
                    else:
                        compare(ts_stats, rust_stats, "workload")
                        compare(normalized(ts["result"]), normalized(rust["result"]))
                        # Ensure repeated processes did not silently change the output.
                        compare(normalized(by_runtime["ts"][0]["result"]), normalized(ts["result"]))
                        row["parity"] = "exact classifications; numeric tolerance 1e-9"
                row["status"] = "ok"
            except (ValueError, AssertionError) as error:
                row.update(status="mismatch", error=str(error))
                failures.append(f"{identifier}: {error}")
        for runtime, reports in by_runtime.items():
            values = [v for report in reports for v in report.get("elapsedMs", [])]
            if values:
                row[runtime] = metrics(values)
                row[runtime]["roundMediansMs"] = [statistics.median(r["elapsedMs"]) for r in reports if "elapsedMs" in r]
                row[runtime]["statsPerBatch"] = reports[0].get("stats")
                row[runtime]["runtime"] = reports[0].get("runtime", "Rust release")
        if row["status"] == "ok":
            row["speedup"] = row["ts"]["medianMs"] / row["rust"]["medianMs"]
        rows.append(row)
    summary = {"metadata": metadata, "scenarios": rows, "failures": failures}
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    lines = ["# Benchmark das pipelines TypeScript e Rust", "",
             f"Execução UTC: {metadata['date']}. CPU: {metadata['cpu']}. "
             f"Bun {metadata['bun']}; Node {metadata.get('node', '')}; {metadata['rustc']}; Rust release (LTO/opt-level 3).", "",
             f"{metadata['rounds']} rodadas em processos novos, alternando a ordem dos runtimes, sem benchmarks concorrentes. "
             "Engine falsa: 3 aquecimentos e 15 amostras por rodada. Stockfish real: 1 aquecimento e 5 amostras por rodada. "
             "Cada amostra mede um lote; os números abaixo são normalizados por partida ou busca.", "",
             "Os tempos incluem parsing de PGN, classificação/fases/abertura, progresso, cache injetado e teardown. "
             "O grupo Stockfish inclui spawn, pipes UCI, busca e espera do encerramento. "
             "Não medem IPC Tauri, renderização nem SQLite; não representam tempo de ponta a ponta no app. "
             "Modos depth/time da engine falsa têm o mesmo fluxo de dados e não aguardam o orçamento nominal.", "",
             "O cache injetado mantém hits estáticos: frio=0%, quente=100%, misto=índices pares. "
             "Escritas são contadas, mas não aquecem rodadas posteriores nem fazem I/O. "
             "Os scores sintéticos exercitam a análise adaptativa e não descrevem a qualidade das partidas reais.", "",
             "Paridade: resultados completos e contadores são comparados na engine falsa, com tolerância absoluta de 1e-9. "
             "No Stockfish real são comparados mainline, posições e contadores de trabalho; avaliações/profundidades podem variar, sobretudo por tempo.", "",
             "TS com engine falsa roda no Bun; TS com Stockfish real roda no Node. "
             "O subprocesso Stockfish encerrou com código 0 antes de responder `uci` no transporte Bun testado, "
             "enquanto o transporte Node funcionou. Isso é uma limitação do harness observado nesta máquina, não um diagnóstico do app. "
             "As razões TS/Rust comparam implementações/runtimes/transportes e não isolam apenas a linguagem.", "",
             "| Cenário | Runtime TS | TS mediana (ms) | Rust mediana (ms) | TS/Rust | TS p95 (ms) | Rust p95 (ms) |",
             "|---|---|---:|---:|---:|---:|---:|"]
    for row in rows:
        if row["status"] == "ok":
            lines.append(f"| {row['id']} | {row['ts']['runtime']} | {row['ts']['medianMs']:.4f} | {row['rust']['medianMs']:.4f} | {row['speedup']:.2f}× | {row['ts']['p95Ms']:.4f} | {row['rust']['p95Ms']:.4f} |")
        else:
            lines.append(f"| {row['id']} | — | {row['status']} | {row.get('rust', {}).get('medianMs', 0):.4f} | — | — | — |")
    lines += ["", "## Partidas reais", "",
              f"Fonte: [{fixture['source']}]({fixture['source']}). Corpus local `app/benchmarks/games.pgn`, sem comentários/análises de terceiros.", "",
              "| ID | Brancas | Pretas | Data | Meios-lances |", "|---|---|---|---|---:|"]
    for identifier, game in fixture["games"].items():
        if identifier.startswith("real"):
            header = game["metadata"]
            lines.append(f"| {identifier} | {header.get('White', '')} | {header.get('Black', '')} | {header.get('Date', '')} | {game['plies']} |")
    lines += ["", "## Memória e limites", "",
              "Pico RSS (MiB) por rodada, capturado por getrusage em wrapper novo; inclui runtime, harness e resultados retidos. "
              "Pode incluir o pico de um descendente aguardado (Stockfish), mas não soma processos: não é a memória total do motor/app. "
              "Heap retido do Bun nos JSONs é diagnóstico e não é diretamente comparável ao RSS Rust.", "",
              f"TS Bun: {metadata['rssMiB']['ts']}. TS Node/Stockfish: {metadata['rssMiB'].get('tsStockfish', [])}. "
              f"Rust (suíte completa): {metadata['rssMiB']['rust']}. Os grupos carregam diferentes volumes de resultados, portanto não são uma comparação isolada de memória da pipeline.", "",
              "FEN inicial/promoção: a referência TS falha no replay; o Rust completa esses cenários. "
              "Não há speedup comparável para eles. Percentis descrevem lotes normalizados, não latência individual de cada evento. "
              "Resultados são desta máquina e do Bun; o WebView do app usa outro runtime JavaScript.", "",
              "Cancelamento usa busca bloqueada com engine falsa e aguarda finalização. "
              "Não mede latência de matar um Stockfish real; o teste de sessão real cobre a correção desse ciclo. "
              "Falhas, timeouts, persistência, navegação, ownership e encerramento são cobertos pela suíte de testes; "
              "não são apresentados como medidas comparativas quando não há harness TS equivalente.", "",
              "## Reprodução", "", "```sh", "cd app", "bun run bench:compare", "```", "",
              "JSONs completos, logs, amostras, RSS e hashes da carga ficam junto deste relatório. "
              "A compilação e a escrita dos relatórios ficam fora das amostras medidas."]
    if failures:
        lines += ["", "## Divergências", ""] + [f"- {failure}" for failure in failures]
    (output / "report.md").write_text("\n".join(lines) + "\n")
    return summary


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--output", type=Path, default=Path("/tmp/engineroom-benchmark"))
    parser.add_argument("--summarize-only", action="store_true")
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error("rounds must be positive")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    fixture_path = APP / "src-tauri/src/review/fixtures/benchmark.json"
    fixture = json.loads(fixture_path.read_text())
    runs = {"ts": [], "rust": []}
    if args.summarize_only:
        metadata = json.loads((output / "metadata.json").read_text())
        for runtime in runs:
            runs[runtime] = [json.loads((output / f"{runtime}-{i}.json").read_text()) for i in range(metadata["rounds"])]
    else:
        print("Compilando o harness Rust em release (fora das medições)...", flush=True)
        build = command(["cargo", "test", "--release", "--lib", "--no-run", "--message-format=json"], APP / "src-tauri")
        executable = next(json.loads(line)["executable"] for line in build.splitlines() if json.loads(line).get("executable"))
        node_script = output / "bench-ts-node.mjs"
        command(["bun", "build", "scripts/bench-pipeline.mjs", "--target=node", f"--outfile={node_script}"])
        cpu = next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")), platform.processor())
        metadata = {
            "date": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "cpu": cpu, "platform": platform.platform(),
            "bun": command(["bun", "--version"]), "node": command(["node", "--version"]), "rustc": command(["rustc", "--version"]),
            "revision": command(["git", "rev-parse", "HEAD"]),
            "dirty": bool(command(["git", "status", "--porcelain"])),
            "fixtureSha256": hashlib.sha256(fixture_path.read_bytes()).hexdigest(),
            "corpusSha256": hashlib.sha256((APP / "benchmarks/games.pgn").read_bytes()).hexdigest(),
            "rounds": args.rounds, "rssMiB": {"ts": [], "tsStockfish": [], "rust": []},
            "stockfish": "Stockfish 18 bundled x86_64-unknown-linux-gnu; Threads=1 Hash=16; depth=8/time=20ms",
        }
        for i in range(args.rounds):
            order = ("ts", "rust") if i % 2 == 0 else ("rust", "ts")
            for runtime in order:
                print(f"Rodada {i + 1}/{args.rounds}: {runtime}; progresso em {output / f'{runtime}-{i}.log'}", flush=True)
                destination = output / f"{runtime}-{i}.json"
                env = os.environ.copy()
                env["BENCH_REPORT_PATH"] = str(destination)
                env.pop("BENCH_SAMPLES", None)
                rss_path = output / f"{runtime}-{i}.rss"
                with (output / f"{runtime}-{i}.log").open("w") as log:
                    if runtime == "ts":
                        env["BENCH_ENGINE"] = "fake"
                        subprocess.run([sys.executable, str(APP / "scripts/bench-process.py"), str(rss_path), "bun", "scripts/bench-pipeline.mjs"], cwd=APP, env=env, stdout=log, stderr=log, check=True)
                        fake = json.loads(destination.read_text())
                        real_path = output / f"ts-stockfish-{i}.json"
                        real_rss = output / f"ts-stockfish-{i}.rss"
                        env["BENCH_ENGINE"] = "stockfish"
                        env["BENCH_REPORT_PATH"] = str(real_path)
                        subprocess.run([sys.executable, str(APP / "scripts/bench-process.py"), str(real_rss), "node", "--expose-gc", str(node_script)], cwd=APP, env=env, stdout=log, stderr=log, check=True)
                        fake["scenarios"].extend(json.loads(real_path.read_text())["scenarios"])
                        destination.write_text(json.dumps(fake, indent=2) + "\n")
                        metadata["rssMiB"]["tsStockfish"].append(round(int(real_rss.read_text().strip()) / 1024, 2))
                    else:
                        subprocess.run([sys.executable, str(APP / "scripts/bench-process.py"), str(rss_path), executable, "benchmark_complete_pipeline", "--ignored", "--nocapture", "--test-threads=1"], cwd=APP, env=env, stdout=log, stderr=log, check=True)
                runs[runtime].append(json.loads(destination.read_text()))
                metadata["rssMiB"][runtime].append(round(int(rss_path.read_text().strip()) / 1024, 2))
        (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    summary = summarize(output, fixture, runs, metadata)
    print(f"Relatório: {output / 'report.md'}", flush=True)
    print(f"Cenários: {len(summary['scenarios'])}; divergências inesperadas: {len(summary['failures'])}", flush=True)
    if summary["failures"]:
        for failure in summary["failures"]:
            print(failure)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
