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

Para comparar triagem e refinamento com uma referência Stockfish de 5s por
posição, execute em `app/src-tauri/`:

```bash
BENCH_REPORT_PATH=/tmp/engineroom-stockfish-quality.json cargo test --lib benchmark_stockfish_review_quality -- --ignored --nocapture --test-threads=1
```

A comparação usa três sequências curtas com FEN (sacrifício, troca e promoção),
os dois perfis automáticos, cache frio e orçamento total de 6 threads / 96 MB.
Registra tempo da pipeline, divergências de classificação e erro médio em pontos
percentuais de avaliação, para triagem e revisão final. Inclui Stockfish, adapter
shell e SQLite em memória; exclui IPC do frontend e renderização.

A referência tem orçamento fixo de 5s por posição, MultiPV 1 e todos os recursos
do pool; não é uma verdade absoluta. Uma amostra por caso permite investigar
divergências; esse corpus pequeno não calibra os pesos da heurística nem
demonstra ganho geral de velocidade ou força. O teste é
ignorado na suíte normal por executar buscas reais com os tempos dos perfis.
