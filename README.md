# EngineRoom

Aplicativo desktop para **revisão de partidas de xadrez** com o motor **Stockfish 18**. Importe um PGN e receba uma análise completa: barra de avaliação, gráfico de avaliação por lance, classificação lance a lance, acurácia por lado, linhas candidatas e detecção de abertura (ECO).

> Toda a análise acontece localmente — seu PGN não sai do seu computador.

## Funcionalidades

- **Importação de PGN** por arrastar e soltar, seletor de arquivos (`.pgn`, `.txt`) ou colagem direta, com validação ao vivo (nomes, Elos, resultado, número de lances).
- **Análise com Stockfish 18** em profundidade fixa por posição, com três níveis de qualidade (Rápido / Equilibrado / Profundo) e de 1 a 5 linhas candidatas por lance.
- **Classificação de lances** no estilo chess.com: Melhor / Excelente / Bom / Imprecisão / Erro / Erro Grave / Livro, com acurácia percentual por cor.
- **Detecção de abertura (ECO)** offline a partir de ~500 códigos (A00–E99), com o dataset incorporado no backend Rust.
- **Tela de revisão** com tabuleiro (Chessground + peças cburnett), seta do melhor lance, barra de avaliação, gráfico SVG navegável, painel de linhas candidatas, lista de lances com badges coloridos e resumo da partida.
- **Navegação por teclado**: ← → (anterior/próximo), Home/End (primeiro/último).
- **Persistência local (SQLite)**: cache de posições com cobertura por profundidade alcançada, orçamento de tempo e MultiPV e histórico de partidas revisadas com reabertura instantânea, reanálise e exclusão.
- **Autoconfiguração do motor**: ajusta `Threads` (núcleos físicos) e `Hash` (~20% da RAM, entre 512 MB e 4 GB) automaticamente.
- **Engine embarcada**: usa exclusivamente o Stockfish 18 distribuído como sidecar, com botão de teste nas configurações.
- **Tema claro/escuro** aplicado antes da pintura para evitar _flash_.

## Stack

| Camada            | Tecnologias                                                                    |
| ----------------- | ------------------------------------------------------------------------------ |
| App               | Tauri **2**                                                                    |
| Backend           | Rust (edição 2021), `tokio`, `rusqlite` (SQLite _bundled_), `sysinfo`, `serde` |
| Frontend          | React **19**, TypeScript **6**, Effect **3**, Vite **8**, Tailwind CSS **4**   |
| Xadrez            | `shakmaty` + `pgn-reader` (análise Rust), `chess.js` + `chessground` (interface)                                |
| Motor             | Stockfish **18** (sidecar baixado em build/dev)                                |
| Testes            | Vitest **4** + Effect TestClock (frontend), `cargo test` (backend)             |
| Gestor de pacotes | **Bun**                                                                        |

## Pré-requisitos

- [Bun](https://bun.sh) e Node.js
- Toolchain Rust (edição 2021) + `cargo`
- [Dependências de sistema do Tauri 2](https://tauri.app/start/prerequisites/) para o seu SO
- Conexão com a internet na primeira execução/build (para baixar o Stockfish)

## Instalação

Todos os comandos abaixo devem ser executados dentro de `app/`.

```bash
# 1. Instalar dependências do frontend
bun install

# 2. Baixar o sidecar do Stockfish 18 para a plataforma atual
node scripts/fetch-stockfish.mjs
# (idempotente: pula se src-tauri/binaries/stockfish-<triple> já existir)
```

## Uso

```bash
# Desenvolvimento (frontend + backend com hot-reload)
bun run tauri dev

# Build de produção (gera instaladores nativos)
bun run tauri build
```

## Scripts disponíveis

| Script          | Comando                                               | Descrição                                       |
| --------------- | ----------------------------------------------------- | ----------------------------------------------- |
| `dev`           | `vite`                                                | Dev server do frontend (porta 1420)             |
| `build`         | `tsc && vite build`                                   | Type-check + build de produção                  |
| `preview`       | `vite preview`                                        | Pré-visualiza o build                           |
| `tauri`         | `tauri`                                               | Pass-through para a CLI do Tauri                |
| `test`          | `vitest run`                                          | Roda os testes do frontend uma vez              |
| `test:watch`    | `vitest`                                              | Testes do frontend em modo _watch_              |
| `typecheck`     | `tsc --noEmit && tsc --noEmit -p tsconfig.tests.json` | Type-check da aplicação e dos testes            |
| `bench:effect`  | `bun scripts/bench-effect.mjs`                        | Benchmark da referência TypeScript com engine falsa       |
| `bench:rust`    | `cargo test --release --lib benchmark_analysis_overhead -- --ignored --nocapture` | Benchmark do pipeline Rust com engine falsa (script muda para `src-tauri/`) |
| `bench:compare` | `python3 scripts/bench-compare.py`                    | Comparação TS/Rust em partidas reais, cache, adaptativo e Stockfish; detalhes em [`app/benchmarks/README.md`](app/benchmarks/README.md) |
| `bench:uci-ipc` | `tauri dev -- -- --bench-uci-ipc`                     | Benchmark do transporte UCI histórico (não da sessão Rust) |

## Testes

```bash
# Frontend: lógica pura, adapters e lifecycle com fakes
bun run lint
bun run typecheck
bun run test

# Backend: núcleo, pipeline, sessões, SQLite e integração com Stockfish real
cargo test                # dentro de app/src-tauri/
```

Para medir o custo do pipeline Rust (engine falsa, sem Stockfish/IPC/renderização):

```bash
bun run bench:rust
```

O teste `real_stockfish_sidecar_cache_history_reopen_and_live_cancellation` usa o
plugin de shell real, SQLite em memória e o Stockfish embarcado. Ele exige o
sidecar e verifica revisão manual/adaptativa, cache, histórico, reabertura,
análise ao vivo e teardown; seus dados não são gravados no histórico do usuário.

## Estrutura do projeto

- `app/src-tauri/src/review/`: núcleo puro de xadrez/acurácia/fases/ECO,
  seleção adaptativa, transporte UCI, pipeline injetável, repository e sessão.
- `app/src-tauri/src/db/`: cache, histórico, migrações e estatísticas SQLite.
- `app/src/lib/review-session.ts`: aquisição scoped da sessão e aplicação de
  eventos ao store; `backend.ts` define o serviço `AnalysisSessions` e
  `tauri-backend.ts` fornece o adapter IPC.
- `app/src/lib/review-protocol.ts`: schemas dos resultados e eventos estruturados.
- `app/src/lib/review-store.ts`: navegação e variações; `use-review.ts` faz a ponte React.
- `app/src/lib/__tests__/reference/`: pipeline TypeScript anterior, conservado
  exclusivamente como referência de regressão/paridade, fora do bundle.
- `app/src/data/eco.json`: dataset único, incorporado no backend Rust.
- `app/src-tauri/src/engine.rs`: framing compartilhado de stdout e benchmark UCI histórico.

## Arquitetura

- **Sessão de análise em Rust**: o backend possui o Stockfish e executa revisão
  manual por tempo/profundidade, triagem/refinamento adaptativos, SAN, fases,
  classificação, acurácia, cache e salvamento. React recebe progresso e resultados.
- **Contrato de sessão**: `review_session_open` registra uma sessão pertencente à
  janela; comandos posteriores solicitam análise ao vivo, cancelamento e fechamento.
  Channels transmitem eventos com ID de sessão, sequência e ID do pedido ao vivo.
  O frontend valida os schemas e descarta respostas antigas.
- **Lifecycle estruturado**: um semáforo Rust é compartilhado entre sessões e o
  probe das configurações. A engine interrompida ou com falha é descartada e sua
  terminação é aguardada antes de outra aquisição. A navegação mantém o pedido
  mais recente. Fechar a sessão aguarda processo, persistência e operações de DB;
  sair do aplicativo aguarda o teardown das sessões.
- **Núcleo puro e I/O injetada**: o pipeline usa `EngineFactory`/`EnginePort` e
  `Repository`. Fakes e fixtures congeladas verificam paridade com o algoritmo
  anterior, enquanto o teste real usa o mesmo adapter shell da produção.
- **Frontend Effect**: aquisição, IPC e cleanup permanecem scoped, com erros
  tagged e schemas nas fronteiras. `review-store.ts` conserva estado e transições;
  nenhum loop de busca, cálculo de revisão ou salvamento roda em React.
- **SQLite compatível**: cache com PK `(fen, reached_depth, multipv)` e contexto
  de origem. Pedidos de profundidade aceitam avaliações suficientemente profundas;
  pedidos por tempo aceitam somente entradas de tempo com orçamento suficiente.
  Consultas são em lote e gravações incrementais usam transações a cada oito
  posições. JSON corrompido vira miss; erros de I/O continuam fatais.
- **Histórico**: a sessão publica o resultado antes de salvar em best-effort.
  `games_get_review_config` normaliza revisões antigas em Rust, preservando o
  formato SQLite/JSON e a reabertura sem nova busca.
- **PGN como fonte de verdade**: o backend revalida a linha principal e respeita
  FEN de início. Metadados são derivados do PGN, sem novos campos duplicados.
  `chess.js` permanece na prévia de importação e nas interações do tabuleiro.
- **Tema pre-paint**: inicializado por script em `index.html`, antes do React.

## Configurações do usuário

Persistidas em `localStorage` na chave `engineroom.settings.v1`:

- **`theme`**: `"dark"` (padrão) ou `"light"`.

## Licença

GPL-3.0-or-later. Veja [LICENSE](LICENSE) e
[avisos de terceiros](THIRD_PARTY_NOTICES.md). Os avisos MIT anteriores foram
preservados em [licenses/LEGACY-MIT.txt](licenses/LEGACY-MIT.txt).
