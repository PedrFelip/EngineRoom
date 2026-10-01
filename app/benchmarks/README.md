# Benchmarks Rust

Execute em `app/`:

```bash
bun run bench:rust
```

O benchmark usa a pipeline Rust em release com engine falsa. Exclui Stockfish,
IPC e renderização; seus resultados não representam o tempo total de revisão.

Para executar os cenários completos em `app/src-tauri/`:

```bash
cargo test --release --lib benchmark_complete_pipeline -- --ignored --nocapture --test-threads=1
```

A carga determinística está em `src-tauri/src/review/fixtures/benchmark.json`.
Para regenerá-la em `app/`:

```bash
bun scripts/generate-benchmark-fixtures.mjs
```

As partidas do corpus estão em `benchmarks/games.pgn`. Os cenários com Stockfish
exigem o sidecar instalado. O benchmark UCI histórico continua disponível via
`bun run bench:uci-ipc`.
