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

## Baseline reproduzível (Linux)

Execute em `app/` para compilar dois binários release de teste e medir cada um
em um processo isolado:

```bash
python3 benchmarks/run.py --engine fake --scenario game-long-depth-pv3-cold --samples 15 --warmups 3 --output /tmp/manual-cold.json
python3 benchmarks/run.py --engine fake --scenario game-long-depth-pv3-warm --samples 15 --warmups 3 --output /tmp/manual-warm.json
python3 benchmarks/run.py --engine fake --scenario adaptive-long-fast-cold --samples 15 --warmups 3 --output /tmp/auto-cold.json
python3 benchmarks/run.py --engine fake --scenario adaptive-long-fast-warm --samples 15 --warmups 3 --output /tmp/auto-warm.json
python3 benchmarks/run.py --engine fake --scenario search-depth-pv3 --samples 15 --warmups 3 --output /tmp/uci.json
python3 benchmarks/run.py --engine stockfish --scenario stockfish-real1-depth-pv1 --samples 5 --warmups 1 --output /tmp/stockfish.json
```

IDs exatos estão no fixture; `BENCH_STOCKFISH` pode apontar para o executável.
O JSON preserva tempos individuais por operação, um resultado completo e
contadores de trabalho por amostra. Para engine falsa, o runner exige resultados e contadores iguais entre
amostras e entre os dois binários; compare esses campos entre revisões também.
Stockfish pode variar: inspecione resultados e contadores antes de comparar.

`latency` não tem o contador de alocações. `allocations` usa a feature test-only
`bench-alloc`: cada par representa chamadas de alloc/realloc bem-sucedidas e
bytes solicitados no intervalo completo da amostra, inclusive setup e teardown;
não é memória retida nem pico. Há custo de atomics e atividade de outras threads,
portanto não use seus tempos como latência. Os números são por lote; divida por
`batch` para comparar por operação.

CPU `wait4` cobre o processo inteiro, aquecimento e infraestrutura, e inclui
CPU de filhos já aguardados (como Stockfish); não separa a engine. RSS é o maior
`VmHWM` observado do processo de teste com o harness, amostrado a cada 10 ms: pode perder um
processo muito curto, não inclui filhos nem mede pico conjunto. Não some picos
individuais. Essas métricas são diagnósticos de processo, não por operação. Fixe
máquina, Rust, binário Stockfish, carga e configuração; guarde JSONs e commit.
Nenhuma medição inclui frontend, IPC Tauri ou SQLite.

O terceiro valor de cada `allocationSamples` é o pico incremental de bytes
solicitados e ainda vivos acima do início da amostra. Não inclui overhead do
alocador, fragmentação, stack ou memória Stockfish. O contador também observa
trabalho de threads Rust de fundo; execute somente este teste no processo.

Para alternar baseline/candidato sem recompilar entre medições, preserve os dois
executáveis de teste de cada versão e passe `--latency-binary /caminho/normal` e
`--allocation-binary /caminho/instrumentado`. Ambos devem usar o mesmo harness.

`experiment.py` executa sete cenários fake em ordem baseline/candidato/candidato/baseline,
compara resultado completo e contadores e salva cada execução mais `summary.json`:

```bash
python3 benchmarks/experiment.py \
  --baseline-normal /tmp/baseline-normal --baseline-alloc /tmp/baseline-alloc \
  --candidate-normal /tmp/candidate-normal --candidate-alloc /tmp/candidate-alloc \
  --output /tmp/comparison
```

Use `--scenario ID` repetidamente para selecionar cenários; `--engine stockfish`
permite os cenários reais. A igualdade exata é exigida apenas para a engine fake.
As distribuições mostram mediana, quartis e extremos; não são intervalos de confiança.
CPU/RSS cobrem o processo inteiro, incluindo aquecimento e preparação do harness.
O pico de bytes vivos é por lote, sem divisão pelo número de operações.

A fase A/B/B/A sem instrumentação termina antes da fase A/B/B/A de alocações
em cada cenário, evitando intercalar a carga do contador com as medições de tempo.

A comparação com dados brutos, ambiente, limites e reprodução está em
[results/2026-10-29/README.md](results/2026-10-29/README.md).
