# Strobe na pipeline de análise (experimental/)

Infraestrutura reproduzível para medir e verificar partes da pipeline de revisão
do EngineRoom em Rust com [Strobe](https://github.com/Hona/strobe), sem depender
da janela nativa. Ela complementa (não substitui) os testes Rust, o
`strobe_review.rs` existente e as cenas headless de `perf_headless.rs`.

## Qual Strobe?

Há projetos com nomes parecidos no GitHub; o contexto deste repositório resolve
a ambiguidade:

- **Correto:** [Hona/strobe](https://github.com/Hona/strobe) (espelho
  [anomalyco/strobe](https://github.com/anomalyco/strobe)), pacote
  [`@hona/strobe`](https://www.npmjs.com/package/@hona/strobe), fixado em
  [`experimental/package.json`](../package.json) como `0.32.0`. É a ferramenta
  de medição/explicação de performance que roda processos, páginas e apps
  nativos. Evidências no
  [README oficial](https://github.com/Hona/strobe/blob/main/README.md):
  `bunx strobe run --base "<cmd>" --head "<cmd>"`, `bunx strobe bench …`, e a
  instrução explícita “**Agents:** read [skill/SKILL.md](https://github.com/Hona/strobe/blob/main/skill/SKILL.md)”.
  O desenho está em [SPEC.md](https://github.com/Hona/strobe/blob/main/SPEC.md).
- **Não é este projeto:** [`rozbb/strobe-rs`](https://github.com/rozbb/strobe-rs)
  é uma implementação do protocolo criptográfico Strobe; `strobe-audio/*` é um
  sistema de áudio. Nenhum dos dois faz instrumentação de processos Rust e
  nenhum aparece nas dependências do repositório.

A [skill oficial](https://github.com/Hona/strobe/blob/main/skill/SKILL.md) foi
lida e seguida nos pontos aplicáveis: usar `cli()` para um binário, rodar
`--check` antes de cronometrar, declarar contadores `metric.*`, ler
`verdict_kind` (não o texto do veredito) e não transformar números de `--check`
em resultado de performance. **`bunx strobe up` não foi executado**: isso
instalaria a skill globalmente e iniciaria o daemon; a skill permanece apenas
como referência versionada neste documento, conforme a versão fixada no lockfile.

## O que existe aqui

| Arquivo | Função |
| --- | --- |
| [`strobe.json`](../strobe.json) | Referência ao schema da config do Strobe 0.32.0. |
| [`pipeline.bench.ts`](pipeline.bench.ts) | Casos determinísticos da pipeline com engine falsa e cache SQLite isolado. |
| [`real-game.bench.ts`](real-game.bench.ts) | Casos com Stockfish real, reaproveitando `crates/engineroom-backend/examples/strobe_review.rs`. |
| [`../crates/engineroom-backend/examples/pipeline_strobe.rs`](../crates/engineroom-backend/examples/pipeline_strobe.rs) | Harness: dirige o `Pipeline` de produção pelas portas públicas `EnginePort`/`EngineFactory`/`Repository` e imprime contadores `strobe:metric`. |
| [`../crates/engineroom-backend/examples/pipeline_strobe_cache.rs`](../crates/engineroom-backend/examples/pipeline_strobe_cache.rs) | Harness de cache parcial e retomada: interrompe uma revisão no barreira, confirma o flush parcial e relê o resultado com um segundo `Pipeline`. |
| [`../crates/engineroom-backend/examples/pipeline_strobe_sessions.rs`](../crates/engineroom-backend/examples/pipeline_strobe_sessions.rs) | Harness de `ReviewSessions`: cancelamento, navegação latest-wins e ciclo de vida com portas e lease falsos. |
| [`fixtures/`](fixtures/) | Fixtures `short.pgn` (8 posições), `long.pgn` (45) e `fen.pgn` (6, com FEN inicial). |
| [`../scripts/test-strobe-harness.py`](../scripts/test-strobe-harness.py) | Testes que validam os contadores de cada estágio, incluindo os três fixtures de cache. |
| [`../scripts/check-strobe.sh`](../scripts/check-strobe.sh) | Comando único: build, testes, `--check` dos benches determinístico e real. |

Os harnesses não alteram o núcleo nem as portas: a engine falsa é implementada
em cada example, o cache é um `SqliteRepository` apontando para um diretório
temporário removido ao final, e o PGN do harness original é o fixture
[`perf/data/immortal.pgn`](data/immortal.pgn) (46 posições, 45 lances). O
harness de cache usa os próprios fixtures e gera um PV **legal para cada FEN**
a partir da posição, para que a asserção de PV teste o transporte UCI e não
repita uma constante como `e2e4` (ilegal fora da posição inicial). Ambos os
harnesses emitem `fake_engine=1`: nenhum deles mede Stockfish.

## Como rodar

A partir de `experimental/`:

```sh
bun install --frozen-lockfile          # instala @hona/strobe 0.32.0 do lockfile
./scripts/check-strobe.sh              # build release + testes + 11 casos --check
```

Para incluir o Stockfish real (o sidecar pode vir do downloader do app principal):

```sh
cd ../app && node scripts/fetch-stockfish.mjs && cd ../experimental
ENGINE_ROOM_STOCKFISH="$PWD/../app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu" \
  ./scripts/check-strobe.sh
```

O script invoca o Strobe a partir da **raiz do repositório**, porque é assim
que ele resolve variantes de diretório/git; os caminhos nos arquivos `.bench.ts`
são relativos à raiz. Para rodar manualmente:

```sh
# raiz do repositório
experimental/node_modules/.bin/strobe bench \
  experimental/perf/pipeline.bench.ts --head . --check

# com Stockfish
ENGINE_ROOM_STOCKFISH=/caminho/stockfish \
  experimental/node_modules/.bin/strobe bench \
  experimental/perf/real-game.bench.ts --head . --check
```

## Cenários e contadores

| Cenário | Estágio da pipeline | Contadores esperados no fixture |
| --- | --- | --- |
| `pgn` | `core::extract` (PGN → posições/FEN) | `positions=46`, `moves=45` |
| `manual` | revisão manual em cache frio | `cached=0`, `searched=45`, `searches=45` |
| `manual-warm` | duas revisões no mesmo SQLite | `cached=45`, `searched=0`, `searches=45` |
| `live` | `Pipeline::live` com feedback de lance | `live=1`, `classifications=1`, `searches=2` |
| `automatic` | perfil `auto-fast` / fila adaptativa | `positions=46`, `moves=45`, `searched=51` |
| `cache-partial-short` | interrompe após 2 buscas, retoma 5 | `fixture_positions=8`, `interrupt_completed=2`, `partial_cache_entries=2`, `resume_searched=5`, `ports=2/2 active=0` |
| `cache-partial-long` | interrompe após 15 buscas (cruza o flush de 8), retoma 30 | `fixture_positions=45`, `interrupt_completed=15`, `partial_cache_entries=15`, `resume_searched=30` |
| `cache-partial-fen` | fixture com FEN inicial; interrompe após 2, retoma 4 | `fixture_positions=6`, `resume_searched=4` |
| `sessions-cancel-analysis` | cancela uma busca em voo e fecha a sessão | `searches=1`, `cancels_acked=1`, `live_completed=0` |
| `sessions-navigation-latest-wins` | 4 intenções, só a última publica resultado | `jobs_enqueued=4`, `searches=2`, `latest_completed=1`, `stale_completed=0` |
| `sessions-lifecycle` | owner isolation, substituição e shutdown idempotente | `sessions_opened=2`, `auth_rejected=2`, `shutdown_rejects=1` |

Todos os cenários de sessões aceitam um PGN opcional como último argumento;
`navigation-latest-wins` exige pelo menos 5 FENs (o PGN inline e o fixture
`short.pgn` satisfazem isso). `acquires=shutdowns`, `active=0`, `permits=1` e
`peak_active=1` valem para os três cenários.

`ports_*` conta **portas da trait** (`EnginePort`/`EngineFactory`), não processos
do sistema: a engine é falsa e `shutdown()` não encerra nenhum PID. O contador
`fake_engine=1` deixa isso explícito no `--json`.

`searches` conta chamadas efetivas à engine; `searched`/`cached` vêm do último
`Event::Progress`. A engine falsa devolve uma avaliação determinística derivada
do FEN, então os números não mudam entre máquinas. As linhas
`strobe:metric <nome>=<valor> [lower|higher]` são lidas pelo alvo `cli()` e
aparecem como `metric.<nome>` no `-json`/no relatório.

## Vereditos e diagnóstico

`--check` responde “o cenário roda e os contadores são lidos”; **não** é um
veredito de performance. Para comparar duas revisões em máquina silenciosa:

```sh
experimental/node_modules/.bin/strobe bench \
  experimental/perf/pipeline.bench.ts --base <ref> --head . --pairs 8
experimental/node_modules/.bin/strobe query <exp_…> insights
experimental/node_modules/.bin/strobe report <exp_…> --format md
```

Leia `series[].primary.verdict_kind` em `--json` (`faster`/`slower`/`same`/
`unclear`/`underpowered`/`refused`); um `unclear` pede mais pares ou menos
ruído, não significa “sem mudança”. `./scripts/check-strobe.sh --timed` roda um
bloco A/A de 8 pares para conferir o método nesta máquina antes de comparar
branches.

## Limitações observadas

- Nesta máquina o Strobe avisou `! Background` (outros processos ocuparam
  18–53% das CPUs) e `i StaleCalibration` (sem calibração; `strobe up` faria
  isso, mas instala skill/daemon e está fora deste escopo). Por isso os números
  acima são de checagem, não vereditos.
- `memory.peak` é recusado (`NotOnThisOs`) nos casos do harness determinístico:
  o Linux carrega o RSS do processo pai no `ru_maxrss`; no bench com Stockfish
  o pico próprio foi alto o bastante para ser lido (320 MB).
- Um runner compartilhado de CI recusa comparações cronometradas (`SharedRunner`);
  `--check` e contadores contra a base continuam disponíveis.
- A engine falsa exercita o transporte UCI e o pipeline, não a busca do
  Stockfish; o bench real cobre isso, mas exige o sidecar. Ele usa `cd
  experimental && …`, então o custo do shell entra na medição.
- Não foi adicionado job de CI nem mudança em `.github/workflows/checks.yml`:
  a integração permanece opt-in, como os testes `#[ignore]` de
  [`perf_headless.rs`](../crates/engineroom-rune/src/perf_headless.rs).
