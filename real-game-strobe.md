# Estudo Strobe: partida histórica com Stockfish real

Segue a referência do README do experimento para cenários com partida real,
Stockfish, cache SQLite isolado e (opcionalmente) janela nativa. A automação
dos casos de pipeline está em [`perf/README.md`](perf/README.md).

## Pipeline real, sem janela

O example já existente
[`crates/engineroom-backend/examples/strobe_review.rs`](crates/engineroom-backend/examples/strobe_review.rs)
dirige o `Pipeline` de produção com `NativeEngineFactory` e `SqliteRepository`.
Ele lê `perf/data/immortal.pgn` (Anderssen × Kieseritzky, 46 posições) e exige
`ENGINE_ROOM_STOCKFISH`. Cada execução cria `target/strobe-runs/<pid>/` e remove
o diretório ao terminar, então o cache fica isolado entre medições.

Da raiz do repositório:

```sh
cargo build --release --locked -p engineroom-backend --example strobe_review
export ENGINE_ROOM_STOCKFISH="$PWD/binaries/stockfish-x86_64-unknown-linux-gnu"

./target/release/examples/strobe_review fresh   # cache vazio: 45 buscas
./target/release/examples/strobe_review warm    # duas passagens, segunda 100% cache

# cenários com banco persistente (diretório pai deve existir)
STROBE_REVIEW_DB=/tmp/er-strobe-review.db ./target/release/examples/strobe_review seed
STROBE_REVIEW_DB=/tmp/er-strobe-review.db ./target/release/examples/strobe_review reuse
```

Os casos equivalentes do Strobe são
[`perf/real-game.bench.ts`](perf/real-game.bench.ts), executados por
`./scripts/check-strobe.sh` quando `ENGINE_ROOM_STOCKFISH` está definido. A
execução local registrou `exp_1d4c98`: `manual-fresh` e `manual-warm` passaram,
com `memory.peak` por volta de 320 MB e `cpu.work` da ordem de 0,3 s. Isso é
evidência de **checagem/descoberta** (uma execução fria, outro processo ocupou
21% das CPUs); não é veredito de performance. Para veredito, use `--pairs 8`
com `--base <ref>` em máquina silenciosa e leia `verdict_kind` no JSON.

O comando dos casos reais contém `cd experimental && …` porque o example
resolve o PGN relativo ao próprio diretório; o custo do shell entra na medição
e é o mesmo nos dois lados de uma comparação.

## Janela nativa (diagnóstico manual)

O app Rune abre uma janela real com `cargo run --locked`; a resolução do
Stockfish em desenvolvimento usa `experimental/resources/stockfish`, a variável
`ENGINE_ROOM_STOCKFISH` ou o sidecar do app Tauri. Com a tela desbloqueada:

```sh
cd experimental
cargo build --release --locked
ENGINE_ROOM_STOCKFISH=/caminho/stockfish \
  ../node_modules/.bin/strobe record \
  exe:target/release/engineroom-rune --native --for 10s
```

Importe/cole o PGN na interface enquanto a gravação corre; depois
`strobe query <rec_…> threads` e `strobe query <rec_…> hotspots` mostram qual
thread e qual função dominam o trabalho. `strobe look exe:…` serve para conferir
que a janela abriu. Isto não foi automatizado no bench: exige interação humana,
display disponível e um gerenciador de composição favorável.

## Limitações

- Com um compositor ativo, o Strobe avisa que não vê presents da janela;
  `frames.*` e latência de interação são recusados. Nesse cenário, meça
  `cpu.work`, `ui.cpu`, `cpu.wakes` e `memory.app.*`.
- Em sessão bloqueada ou sem `DISPLAY`, o Strobe usa compositor Wayland
  headless; métricas de tela continuam recusadas (`Headless`).
- Comparações cronometradas exigem runner dedicado; em CI compartilhado o
  Strobe recusa com `SharedRunner`, mas `--check` e contadores contra a base
  funcionam.
- O harness determinístico (`pipeline_strobe`) não substitui estes cenários: a
  engine falsa não mede o custo real do NNUE, do processo Stockfish nem do
  encadeamento UCI com o motor nativo.
