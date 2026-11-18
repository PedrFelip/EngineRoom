# Contrato de medição Strobe — tempo da aplicação, Stockfish real e renderização

Este documento é o contrato de nomes, unidades, escopos e interpretação para os
cenários Strobe do experimento Rune. Ele **não** é um relatório de performance e
**não** substitui `perf/README.md`, `strobe-study.md` nem `real-game-strobe.md`:
ele fixa o vocabulário comum para que os cenários novos (`pipeline_strobe_cache.rs`,
`pipeline_strobe_sessions.rs`) possam ser integrados sem trocar métricas
incomparáveis. A referência normativa é a
[skill oficial do Hona/strobe](https://github.com/Hona/strobe/blob/main/skill/SKILL.md),
lida na versão fixada `@hona/strobe 0.32.0`.

Regra de ouro: **uma frase de conclusão só pode usar métricas do mesmo escopo.**
Misturar `cpu.work` de um harness com engine falsa e de um com Stockfish real é
comparar árvores de processos diferentes.

---

## 1. O que cada ferramenta mede

| Ferramenta / alvo | O que é | O que **não** é |
| --- | --- | --- |
| `cli()` + `ctx.exec()` em `.bench.ts` | Um processo tocado até o exit; mede CPU da árvore, wall, memória, contadores impressos | Não mede quadros, latência de input nem GPU |
| `strobe run --check --head "<cmd>"` | Checagem de um comando arbitrário (ex.: `cargo test --ignored` de render headless) | Não é veredito; não atribui hotspots |
| `record exe:<path> --native` / `pid:<n>` | Attribution de processo nativo real (janela ou `--windowless`) | Exige interpretação manual; PID muda a cada execução |
| Harness Rust (`#[ignore]` + `cargo test`) | Prova de comportamento e, se usar `NativeEngineFactory`, prova de encerramento de processo OS | Não mede renderização; não é bench de CPU |

---

## 2. Inventário de métricas (unidade e escopo)

Convenção de unidade do Strobe: texto usa microssegundos por padrão (`--unit ms`
/ `--unit s` só mudam a impressão); **`--json` / `result.json` trazem números em
milissegundos** para métricas de tempo. Contadores `metric.*` são inteiros sem
unidade. Tamanhos são binários: KB = 2¹⁰, MB = 2²⁰, GB = 2³⁰ bytes.

| Métrica | Unidade | Escopo | Origem / disponibilidade |
| --- | --- | --- | --- |
| `run.duration` | ms (µs no texto) | wall da **execução inteira** do `ctx.exec` / comando, spawn → exit | `cli()` e `run`. Se o comando contém `cd experimental && …` (ShellLine), o startup do shell entra no número — é constante entre lados, mas infla o wall |
| `cpu.work` | ms de CPU | **soma de CPU de todos os processos e threads da árvore do alvo**, inclusive filhos Stockfish iniciados durante a execução | `cli()` e nativo. Processos que terminaram durante a run têm sua CPU mantida aqui. É por isso que **não é comparável entre engine falsa e real** |
| `cpu.wakes` | contagem | trocas de contexto da árvore | Indisponível no Windows. Sobe com muitos workers disputando CPU; útil para detectar sobreposição |
| `ui.cpu` | ms de CPU | somente a thread de UI de um app nativo | Non-Windows/native; recusado para `cli()` e para headless |
| `cpu.main` | ms de CPU | somente a thread principal (script/host) | Não usar para comparar engine host vs engine nativa (`NotComparable` entre engines) |
| `memory.peak` | bytes | pico RSS do processo `cli()` | **Recusado `NotOnThisOs` no harness determinístico em Linux**: `ru_maxrss` carrega o RSS do pai. Foi legível só no bench com Stockfish (pico ~320 MB) — tratar como descoberta, não como meta |
| `memory.app.private` / `memory.app.host` | bytes | memória do app nativo (host) | Nativo; no Windows `memory.app.private` pode incluir VRAM mapeada — citar `memory.app.host` |
| `frames.*`, `interaction.p95` | ms | apresentação / latência de input | **Recusado `Headless`** e por compositor ativo; ver `strobe-study.md` e `real-game-strobe.md` |
| `metric.<nome>` | contagem inteira | uma execução do harness, lida das linhas `strobe:metric <nome>=<valor> [higher]` | Declarar no `metrics:` do caso; `cli()` ignora métricas não declaradas |

### 2.1 Contadores canônicos já existentes (não renomear)

`positions`, `moves`, `cached`, `searched`, `searches`, `live`, `classifications`.
Semântica congelada: `searches` conta chamadas efetivas à engine ao longo da vida
do harness; `cached`/`searched` vêm do **último** `Event::Progress`; os demais são
contagens de eventos/posições da execução. Direções (`higher`/`lower`) estão
fixadas em `scripts/test-strobe-harness.py` e devem permanecer iguais.

### 2.2 Contadores dos harnesses de cache e sessões

Os examples `pipeline_strobe_cache.rs` (cenários `short`, `long`, `fen`) e
`pipeline_strobe_sessions.rs` (cenários `cancel-analysis`,
`navigation-latest-wins`, `session-lifecycle`) emitem as linhas `strobe:metric`
abaixo. Todos são **contagens por execução**, sem unidade, e já estão
declarados no `metrics:` de cada caso do `.bench.ts` e espelhados em
`scripts/test-strobe-harness.py`.

| Example | Contadores emitidos | Leitura |
| --- | --- | --- |
| cache | `fixture_positions`, `fixture_moves`, `fixture_terminals`, `fixture_search_positions` | Formato da fixture; `terminals` e `search_positions` são `lower` (são custo/estrutura, não meta) |
| cache | `interrupt_after`, `interrupt_go`, `interrupt_completed`, `interrupt_progress_searched`, `interrupt_progress_cached` | Onde a interrupção aconteceu e o que o último `Progress` viu |
| cache | `partial_cache_entries`, `resume_cache_hits`, `resume_searched`, `resume_go` | Prova de cache parcial + retomada; `resume_go` é o custo evitado |
| cache | `result_positions`, `result_moves`, `deterministic_ok` | Integridade do resultado e determinismo da engine falsa |
| cache | `ports_acquired`, `ports_terminated`, `ports_active`, `ports_peak_active`, `teardown_ok` | **Atenção:** são contadores de *porta* (trait), não de processo OS — ver §7 |
| cache | `fake_engine=1` | Marcador explícito de que a execução é engine falsa |
| sessões | `fake_engine=1` | Marcador explícito de que a execução é engine falsa |
| sessões | `sessions_opened`, `jobs_enqueued`, `searches`, `acquires`, `shutdowns` | Volume de trabalho; `shutdowns` é contagem de chamada ao trait |
| sessões | `peak_active`, `active`, `permits` | Concorrência e lease; `peak_active`/`active` são `lower` |
| sessões | `live_started`, `live_completed`, `latest_completed`, `stale_completed`, `stale_events`, `accepted_events`, `max_sequence` | Latest-wins: só o request corrente pode completar; `stale_completed` deve ser 0 |
| sessões | `cancels_acked`, `closes_acked`, `repo_drained` | Cancelamento/fechamento **acknowledged after cleanup** |
| sessões | `auth_rejected`, `shutdown_rejects`, `wrong_session`, `sequence_violations` | Eventos inválidos rejeitados; `wrong_session`/`sequence_violations` devem ser 0 |

Os dois examples emitem o **mesmo** marcador `fake_engine=1`, declarado no
`metrics:` de cada caso do `.bench.ts`. Isso torna explícito que nenhum dos dois
mede Stockfish — evitando que `cpu.work`/`searches` sejam lidos como custo de
engine real. O PV da engine falsa no harness de cache é um lance legal derivado
do FEN; no de sessões é um sentinela fixo, pois esse harness só afirma
contadores, nunca valores de PV.

Fixtures ficam em `perf/fixtures/short.pgn`, `long.pgn` e `fen.pgn`. O nome da
fixture entra no **nome do caso** (`cache-partial-short`, `cache-partial-long`,
…), nunca como um `metric.*` diferente — assim o mesmo `metrics:` serve para as
três variantes. As interrupções acontecem após 2/15/2 buscas; `long` cruza o
flush periódico de 8 entradas, e `scripts/test-strobe-harness.py` também testa
uma interrupção exatamente em 8 para cobrir esse limite.

---

## 3. Engine falsa × trabalho real

| Aspecto | Engine falsa (`pipeline_strobe*`) | Stockfish real (`strobe_review.rs`, `real-game.bench.ts`, testes `#[ignore]`) |
| --- | --- | --- |
| Processo OS | **Nenhum**. `FakePort::send` só preenche uma `VecDeque` | `NativeEngineFactory` → `tokio::process::Command::spawn` com stdin/stdout pipe |
| NNUE / busca | Não existe; `deterministic_cp(fen)` deriva o score do hash do FEN | Real; `go depth 8` ou `go movetime …` executa a busca |
| `cpu.work` mede | Rust do pipeline + SQLite do harness | Rust + SQLite **+ todos os processos Stockfish filhos** |
| `searches` | Nº de chamadas `go`, barato e determinístico | Nº de chamadas `go`, cada uma com custo de CPU real |
| Rede / banco | SQLite isolado em temp dir removido no fim | SQLite isolado, ou `STROBE_REVIEW_DB` persistente nos cenários `seed`/`reuse` |
| Encerramento | `shutdown()` da fake é `async {}` — **não encerra nada** | `shutdown()` faz `start_kill()` + `wait().await` (experimental) ou `kill()` + espera `CommandEvent::Terminated` (Tauri) |
| Pré-requisito | Nenhum | `ENGINE_ROOM_STOCKFISH` apontando para o binário |

**Interpretação obrigatória:** um cenário com engine falsa pode provar correção de
pipeline, cache, cancelamento e contadores; **não pode** provar custo de NNUE,
encadeamento UCI com o motor nativo, nem encerramento de processo OS. É exatamente
a limitação já registrada em `real-game-strobe.md` ("a engine falsa não mede o
custo real do NNUE, do processo Stockfish nem do encadeamento UCI").

---

## 4. Sobreposição de workers — nunca subtrair somas concorrentes do wall

Perfis automáticos usam N workers Stockfish simultâneos sob **uma** permissão de
lease. Consequências para leitura de números:

1. `cpu.work` é uma **soma sobre threads/processos**. Com N workers ocupados,
   `cpu.work` pode chegar a ≈ N × `run.duration` (ou mais, se threads de
  do sistema somarem). Isso é paralelismo, não regressão.
2. **Nunca** calcular "tempo da aplicação" como `run.duration − cpu.work`, nem
   somar `cpu.work` de workers concorrentes e chamar isso de wall. Soma de CPU
   concorrente não é tempo decorrido.
3. Para separar as parcelas, use escopos, não subtração:
   - **Wall da aplicação** → `run.duration` (ou `cli.launch`/fases, quando houver).
   - **CPU total da árvore** → `cpu.work` (inclui Stockfish).
   - **CPU da engine isolada** → `cpu.work` do cenário real **menos** `cpu.work`
     do mesmo cenário com engine falsa *somente como estimativa grosseira de
     descoberta*, nunca como número de veredito (os escopos diferem: um tem
     filhos, o outro não). Melhor: `record exe:… --native` e `query <rec>
     hotspots --pid <pid|exe>`, que atribuem CPU por processo.
   - **Parcela de UI/render** → cenário headless separado (§5), nunca o bench
     de pipeline.
4. Para comparar um **custo aditivo** esperado (uma thread a mais, um flush a
   mais) entre base e head, use o `shift` do Strobe (head − base **dentro dos
   pares**), não a diferença das medianas de cada lado.
5. `cpu.wakes` é o sinal de alerta para contenção: se sobe muito junto com N
   workers, a "melhoria" de wall pode ser só escalonamento.

---

## 5. Três bancos separados, um veredito por pergunta

O app Tauri (`app/`) fica fora destes bancos: a evidência dele são os testes Rust
de sessão/sidecar, executados por `cargo test` no checkout do app. Aqui há apenas
três perguntas — pipeline determinística, trabalho Stockfish real e construção
da cena headless. Uma frase de conclusão não cruza esses escopos.

| Pergunta | Banco / comando | Métrica primária | Com Stockfish? |
| --- | --- | --- | --- |
| "A pipeline de cache/cancelamento está correta e barata em Rust?" | `perf/pipeline.bench.ts` (`cli()` fake engine) | `cpu.work`, com contadores `metric.*` | Não |
| "Quanto custa a revisão real?" | `perf/real-game.bench.ts` (`cli()` + `NativeEngineFactory`) | `cpu.work`, `run.duration`, `memory.peak` (quando legível) | Sim |
| "Quanto custa construir a cena de revisão?" | `perf_headless.rs` `#[ignore]` via `strobe run --check` | `cpu.work`, `run.duration`; **`frames.*` recusado** | Não |

Regras:

- Um `.bench.ts` por pergunta. Não colocar caso de render no bench de pipeline,
  nem caso real no bench determinístico — o bench determinístico roda em CI
  sem sidecar.
- Render headless não tem apresentação: `frames.*`, latência de input e GPU são
  recusados (`Headless`; com compositor ativo, `frames.*` também é recusado).
  Usar `cpu.work`/`run.duration` e, para hotspots, gravar o **binário de teste**
  diretamente (`target/debug/deps/engineroom_rune-*`, cujo hash muda) com
  `record exe:<path> --native`. `cargo test` inclui o custo do próprio cargo.
- `memory.peak` recusado no harness determinístico é uma resposta, não falha de
  setup: não trocar a primária por ele só para "ter número".

---

## 6. `--check` vs veredito

| | `--check` | Comparação cronometrada |
| --- | --- | --- |
| O que faz | Roda cada caso **uma vez** por lado; prova que o caso roda e que cada métrica declarada dá valor ou recusa | ≥ 8 pares, `--base <ref> --head .`, máquina silenciosa |
| Veredito? | **Não.** Número de check é descoberta | Sim, em `series[].primary.verdict_kind` do `--json` |
| Exit | 0 se todos passam; 1 se um caso falha; 4 se todos os primários recusados | 0 mesmo em `slower` — **ler `verdict_kind`**, não a palavra impressa |
| `--noworse` | Exits 2 com `--check` em comandos: cair `--check` e usar `--noworse cpu.work=2%` | `--noworse 2%` para refactor; `--noworse count.x=0%` para contador exato |
| `--base` com `--check` | Só imprime base e head lado a lado; "nothing compared" | — |

Leitura de `verdict_kind` (`--json`): `faster`/`slower` (ou `less`/`more`,
`fewer`), `same` (±2%), `unclear` (pede mais pares ou menos ruído — **não**
significa "sem mudança"), `underpowered`, `refused`. `--require-verdict` torna
5 se undecidido e 4 se algum primário recusado. `unclear`/`refused` são
resultado, não "o benchmark não funcionou"; corrigir o setup quando a recusa
nomeia validador (`Background`, `SharedRunner`, `Throttled`, `Headless`,
`NotOnThisOs`, `NoCounter`, …), nunca contornar.

O ruído local observado (`! Background` com 18–53% de CPUs ocupadas,
`i StaleCalibration`) torna os números existentes **checagem/descoberta**. Para
veredito: máquina silenciosa, `--pairs 8`, `--base <ref>`, ler `verdict_kind`.
Em runner compartilhado de CI, comparação cronometrada é recusada
(`SharedRunner`); `--check` e contadores contra a base continuam disponíveis.

---

## 7. Validação de teardown: processo real vs portas falsas

Critérios de evidência, do mais fraco ao mais forte:

1. **Porta falsa** (`FakePort::shutdown` = `async {}`): prova apenas que o
   código chamou o método. **Nunca satisfaz** "processo encerrado".
2. **Contador de `shutdown()`** (`teardown_acks`, `engines_shutdown`): prova que
   a chamada foi feita e, se o future foi aguardado, que a implementação
   terminou. Não prova que um PID sumiu do OS.
3. **Liberação do lease** (`permit.available_permits() == 1`, ou `acquire()`
   bem-sucedido após o teardown): é um **proxy forte**, *desde que* a
   implementação só devolva a permissão depois de `wait().await` (experimental,
   `native_engine.rs:126-129/166-169`) ou depois de `CommandEvent::Terminated`
   (Tauri, `review/engine.rs:237-253`). Ainda é um proxy: não nomeia o PID.
4. **Evidência de OS**: o PID do Stockfish não existe mais em `/proc/<pid>` (Linux)
   ou `try_wait()`/equivalente retorna "exited". **É este o critério exigido
   quando a pergunta é "o processo foi encerrado?".**

Cobertura existente (levantada neste repositório):

- `experimental/crates/engineroom-uci/tests/native_engine.rs:23`
  `real_native_stockfish_search_and_exclusive_pool_teardown` — `#[ignore]`,
  exige `ENGINE_ROOM_STOCKFISH`. Faz busca real (depth ≥ 4, PV não vazio),
  desliga uma porta e confirma que `acquire()` ainda bloqueia (a outra porta
  retém o lease), desliga a segunda e confirma que `acquire()` sucede.
  **É o nível 3:** forte, mas indireto.
- `experimental/crates/engineroom-uci/tests/native_engine.rs:67` e `:97` —
  fixtures de shell (`/bin/sh`, `exec sleep 60`) testam framing parcial,
  stderr drenado, timeout de stdin e cancelamento; são portas **não reais**
  para fins de "processo Stockfish encerrado". O teste `:97` chega a pollar o
  reaper e abandonar o waiter, mas o encerramento observado é de `sleep`.
- `app/src-tauri/src/review/session/tests.rs:276`
  `real_stockfish_sidecar_cache_history_reopen_and_live_cancellation` —
  `#[ignore]` na prática (exige sidecar linkado). Cobre cache, histórico,
  reabertura, live cancel, nova abertura da janela e reaquisição do lease pela
  porta de Settings. Afirma `permit.available_permits() == 1` e
  `stopped >= 5` (contagem de `shutdown()` via `RecordingPort`). **Nível 3.**
- `app/src-tauri/src/review/tests/pool.rs:208`
  `real_sidecar_pool_holds_one_lease_until_all_processes_terminate` — pool de 3
  processos reais, `acquire()` bloqueia até `pool.close()`, depois probe reusa o
  lease. **Nível 3.**
- `app/src-tauri/tests/engine_handshake.rs:52`
  `stockfish_uci_handshake_and_bestmove` — spawn direto, `uciok`, `id name`,
  `bestmove`; dá `kill()` no fim (`:114-115`), mas **não** afirma exit/terminação.
  **Nível 1–2.**
- `app/src-tauri/src/review/tests/benchmark.rs:127-214` (`RealFactory`/`RealPort`,
  usado pelos benchmarks `#[ignore]` `:394`, `:558`) — `shutdown()` faz
  `kill()` e depois `spawn_blocking(child.wait().unwrap())` + `reader.join()`
  (`:202-211`). **Nível 4** (espera real), mas só em benchmarks opt-in.
- `experimental/crates/engineroom-rune/src/bridge/tests.rs:427, 517, 792, 849`
  — quatro testes `#[ignore]` com Stockfish real (linhas ao vivo, cancelamento,
  troca de workspace, retomada por cache). Nenhum afirma encerramento de PID.
  **Nível 1–3** conforme o que checam.
- Antes deste trabalho **não existia** nenhum `try_wait`/`wait()`/process-gone
  assertion em `experimental/**/tests`, nem qualquer teste que comparasse PIDs
  do `/proc` — toda a evidência de encerramento era indireta (lease).
- `crates/engineroom-backend/examples/strobe_review.rs` — bench real, sem
  asserção de encerramento de PID; `cli()` do Strobe **falha a run se um
  processo ficar vivo**, o que é um sinal útil, mas não é prova de PID.

**Gate de aceite para a entrega de lifecycle real:** um teste `#[ignore]` que
rode com `ENGINE_ROOM_STOCKFISH`, tire um *snapshot* dos PIDs Stockfish em
`/proc` antes de spawnar, confirme os novos PIDs após o spawn, execute busca
real, acione `shutdown()`/cancele a intenção, e afirme (com poll curto) que
**todos os novos PIDs desapareceram** e que o lease foi devolvido. Esse teste
está implementado em
`experimental/crates/engineroom-uci/tests/real_process_lifecycle.rs` (arquivo
exclusivo, sem dependência dos examples dos outros agentes).

---

## 8. Recomendações integráveis ao pai

1. Os nomes da §2.2 são emitidos pelos examples e já estão espelhados no
   `metrics:` de cada caso **e** em `scripts/test-strobe-harness.py`,
   incluindo `fake_engine=1` nos dois harnesses.
2. Manter `cpu.work` como primária nos benches de pipeline e real. Não trocar
   por `run.duration` só porque "parece mais estável": são perguntas diferentes.
3. Declarar explicitamente em cada `.bench.ts` que `cpu.work` do bench real
   inclui Stockfish e o do bench fake não — evitar comparação cross-scenario.
4. Rodar o gate da §7 (teste real de lifecycle) antes de aceitar qualquer
   mudança em cancelamento/latest-wins/teardown. Engine falsa sozinha não
   fecha essa pergunta.
5. Não promover números de `--check` a tabela de resultados. Usar `strobe
   report experiment <exp> --format pr --update-pr <n>` com o bloco gerado,
   nunca números redigitados.
6. Não criar job de CI novo sem decidir o escopo: bench determinístico roda em
   CI; comparação cronometrada exige runner dedicado e `--check --base …` com
   contador `--noworse count.x=0%` em runner compartilhado.

---

## 9. Comandos de reprodução

Da raiz do repositório:

```sh
# Pré-requisito único (Strobe 0.32.0 fixado no lockfile)
cd experimental && bun install --frozen-lockfile && cd ..

# 1) Checagem determinística (engine falsa) — sem Stockfish
experimental/node_modules/.bin/strobe bench \
  experimental/perf/pipeline.bench.ts --head . --check

# 2) Checagem real (Stockfish) — exige sidecar
SF="$PWD/app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu"
ENGINE_ROOM_STOCKFISH="$SF" experimental/node_modules/.bin/strobe bench \
  experimental/perf/real-game.bench.ts --head . --check

# 3) Gate de lifecycle real (evidência de PID encerrado) — #[ignore]
cd experimental
ENGINE_ROOM_STOCKFISH="$OLDPWD/app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu" \
  cargo test --locked -p engineroom-uci real_process_lifecycle -- --ignored --nocapture

# 4) Render headless (separado; frames.* recusado)
cargo test --locked -p engineroom-rune perf_headless::rebuild_review_frames -- --ignored --exact
../experimental/node_modules/.bin/strobe run --check --head \
  "cargo test --locked -p engineroom-rune perf_headless::rebuild_review_frames -- --ignored --exact"
cd ..

# Script único: quando ENGINE_ROOM_STOCKFISH é definido, também roda o gate §3
ENGINE_ROOM_STOCKFISH="$SF" experimental/scripts/check-strobe.sh

# 5) Veredito (somente em máquina silenciosa; ler verdict_kind no --json)
experimental/node_modules/.bin/strobe bench \
  experimental/perf/pipeline.bench.ts --base <ref> --head . --pairs 8 --json
experimental/node_modules/.bin/strobe query <exp_…> insights
```

---

## 10. Avisos conhecidos nesta máquina (não são falhas)

- `! Background`: outros processos ocuparam 18–53% das CPUs; `--check` ainda
  corre, mas os números são de máquina carregada.
- `i StaleCalibration`: sem `strobe up`; calibração não foi feita de propósito
  (instalaria skill/daemon globalmente).
- `memory.peak` recusado `NotOnThisOs` no harness determinístico; legível no
  bench com Stockfish porque o pico próprio foi maior.
- Compositor ativo/sessão travada: `frames.*` e latência recusados
  (`Headless`/`SessionLocked`); medir `cpu.work`, `ui.cpu`, `cpu.wakes`,
  `memory.app.*`.
- `cd experimental && …` no bench real: custo do shell entra no wall dos dois
  lados; é constante, mas infla `run.duration` relative ao bench fake.
- `HarnessOverlay`: arquivos em `perf/` são copiados para a base; manter o
  código sob teste fora de `perf/` para cada lado rodar o seu próprio.

---

## 11. Validação executada para esta entrega

Comandos executados a partir de `experimental/` neste checkout (o sidecar veio
de `app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu`):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/test-strobe-harness.py
ENGINE_ROOM_STOCKFISH="$PWD/../app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu" \
  cargo test --locked -p engineroom-uci real_process_lifecycle -- --ignored --nocapture
ENGINE_ROOM_STOCKFISH="$PWD/../app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu" \
  cargo test --locked -p engineroom-uci real_native_stockfish -- --ignored
ENGINE_ROOM_STOCKFISH="$PWD/../app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu" \
  ./scripts/check-strobe.sh
./scripts/check-headless.sh
./scripts/check-responsive.sh
```

Resultados observados:

- `fmt`, `clippy -D warnings` e `cargo test --workspace` limpos. Os testes
  `#[ignore]` continuam ignorados na suíte normal — não são contados como
  passados.
- Python: **16 testes passaram** (7 do harness original, 4 de cache, 5 de
  sessões), incluindo o limite de flush em 8 e a rejeição de PGN com menos de
  5 FENs na navegação.
- `real_process_lifecycle_shutdown_and_cancel_reap_stockfish_pids`: **1 passou
  (~0,7 s)**. Dois processos reais com `go depth 4` tiveram todos os PIDs
  removidos após `shutdown()`, o lease foi readquirido por uma nova porta, e
  um `go movetime 30000` cancelado terminou com
  `ReviewErrorCode::Cancelled` e PID removido. É o nível 4 da §7: nenhuma
  engine falsa participa.
- `real_native_stockfish_search_and_exclusive_pool_teardown`: **1 passou
  (~0,3 s)**; é o nível 3 pré-existente (proxy do lease), não evidência de PID.
- `strobe bench pipeline.bench.ts --head . --check`: **11/11** casos passaram;
  `real-game.bench.ts --head . --check`: **2/2**, com `memory.peak` legível
  (~320 MB). `! Background` (14–23% das CPUs) e `i StaleCalibration` estiveram
  presentes: são números de checagem/descoberta, **não** veredito.
- `check-headless.sh` e `check-responsive.sh` passaram (matriz de capturas e
  limites de layout em 720×560 a 1440×1080).
- Varredura final de processos não encontrou nenhum `stockfish*` residual.

Created with Hy4 flash Free in OpenCode.
