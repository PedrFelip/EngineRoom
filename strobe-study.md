# Estudo Strobe: cena de revisão headless

Este documento fecha a referência do README do experimento para a medição da
cena de revisão **sem janela**. A infraestrutura da pipeline em si está em
[`perf/README.md`](perf/README.md); aqui o foco é o custo de CPU da interface
Rune em modo headless.

## O que é medido

Os testes opt-in em
[`crates/engineroom-rune/src/perf_headless.rs`](crates/engineroom-rune/src/perf_headless.rs)
rodam o runtime Rune real com o PGN histórico `perf/data/immortal.pgn`:

- `import_and_open_real_game` — quatro aberturas da partida;
- `navigate_real_game` — 45 lances, ida ao início, duas vezes;
- `rebuild_review_frames` — 100 reconstruções da cena de revisão;
- `serve` — driver interativo que imprime `STROBE_READY`, lê comandos
  (`import`, `navigate`, `reset`, `rebuild`, `quit`) da stdin e responde
  `STROBE_DONE`.

Eles são `#[ignore]` justamente para não entrarem no `cargo test` normal. O
harness de pipeline (`perf/pipeline.bench.ts`) mede as fases de análise; estes
medem a construção da cena, sem Stockfish e sem banco.

## Reproduzir

Da raiz do repositório:

```sh
cargo test --locked -p engineroom-rune perf_headless::rebuild_review_frames \
  -- --ignored --exact                       # só o teste, sem Strobe

../node_modules/.bin/strobe run --check \
  --head "cargo test --locked -p engineroom-rune perf_headless::rebuild_review_frames -- --ignored --exact"
```

O `strobe run --check` usa o alvo de comando e reporta `cpu.work`, duração e
pico de memória; a execução local deste estudo registrou
`exp_66f160` com `cpu.work 473.638 µs` — número de **checagem**, não veredito.
Para comparar uma mudança de layout, use duas linhas de comando com
`--base`/`--head` e ao menos oito pares em máquina silenciosa; leia
`verdict_kind` em `--json`, não a palavra impressa.

O driver `serve` permite correlacionar um comando com um trecho de uma gravação:
inicie o processo do teste, espere `STROBE_READY`, envie um comando e anote a
linha `STROBE_DONE`; um `strobe record pid:<pid>` paralelo captura CPU e
attribution. Isso é diagnóstico manual: o PID muda a cada execução e o
comando de teste não é um executável estável, então não virrou caso de bench.

## Limitações

- Rune headless usa CPU, não apresentação: `frames.*`, latência de interação e
  GPU não são métricas confiáveis aqui. Prefira `cpu.work`/`run.duration`; a
  skill oficial registra a recusa `Headless` para quadros em páginas headless.
- `cargo test` inclui o custo do próprio cargo e do harness; para atribuir
  hotspots de verdade é preciso gravar o binário de teste diretamente
  (`target/debug/deps/engineroom_rune-*`, cujo hash muda) com
  `strobe record exe:<path> --native`, e símbolos resolvidos ajudam.
- A máquina local mostrou `! Background` e `i StaleCalibration`; sem calibração
  e com outras cargas, `--check` ainda é útil, mas comparações cronometradas
  podem ser recusadas como `Throttled`/`Background`.
- As cenas `--scene`/`--snap`/`--tree` descritas no README raiz permanecem
  determinísticas e não abrem Stockfish nem banco; elas são verificadas por
  `scripts/check-headless.sh`, não por este estudo.
