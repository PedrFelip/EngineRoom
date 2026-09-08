# EngineRoom

Aplicativo desktop para **revisão de partidas de xadrez** com o motor **Stockfish 18**. Importe um PGN e receba uma análise completa: barra de avaliação, gráfico de avaliação por lance, classificação lance a lance, acurácia por lado, linhas candidatas e detecção de abertura (ECO).

> Toda a análise acontece localmente — seu PGN não sai do seu computador.

## Funcionalidades

- **Importação de PGN** por arrastar e soltar, seletor de arquivos (`.pgn`, `.txt`) ou colagem direta, com validação ao vivo (nomes, Elos, resultado, número de lances).
- **Análise com Stockfish 18** em profundidade fixa por posição, com três níveis de qualidade (Rápido / Equilibrado / Profundo) e de 1 a 5 linhas candidatas por lance.
- **Classificação de lances** no estilo chess.com: Melhor / Excelente / Bom / Imprecisão / Erro / Erro Grave / Livro, com acurácia percentual por cor.
- **Detecção de abertura (ECO)** offline a partir de ~500 códigos (A00–E99), via dataset dinâmico para não inflar o bundle.
- **Tela de revisão** com tabuleiro (Chessground + peças cburnett), seta do melhor lance, barra de avaliação, gráfico SVG navegável, painel de linhas candidatas, lista de lances com badges coloridos e resumo da partida.
- **Navegação por teclado**: ← → (anterior/próximo), Home/End (primeiro/último).
- **Persistência local (SQLite)**: cache de posições por `(fen, mode, value, multipv)` e histórico de partidas revisadas com reabertura instantânea, reanálise e exclusão.
- **Autoconfiguração do motor**: ajusta `Threads` (núcleos físicos) e `Hash` (~20% da RAM, entre 512 MB e 4 GB) automaticamente.
- **Engine embarcada**: usa exclusivamente o Stockfish 18 distribuído como sidecar, com botão de teste nas configurações.
- **Tema claro/escuro** aplicado antes da pintura para evitar _flash_.

## Stack

| Camada            | Tecnologias                                                                    |
| ----------------- | ------------------------------------------------------------------------------ |
| App               | Tauri **2**                                                                    |
| Backend           | Rust (edição 2021), `tokio`, `rusqlite` (SQLite _bundled_), `sysinfo`, `serde` |
| Frontend          | React **19**, TypeScript **6**, Effect **3**, Vite **8**, Tailwind CSS **4**   |
| Xadrez            | `chess.js` (PGN/FEN), `chessground` (tabuleiro)                                |
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
| `bench:effect`  | `bun scripts/bench-effect.mjs`                        | Benchmark determinístico com engine falsa       |
| `bench:uci-ipc` | `tauri dev -- -- --bench-uci-ipc`                     | Benchmark E2E do transporte IPC de comandos UCI |

## Testes

```bash
# Frontend: lógica pura, adapters e lifecycle com fakes
bun run lint
bun run typecheck
bun run test

# Backend (testes unitários em src/db.rs, src/system.rs + teste de integração)
cargo test                # dentro de app/src-tauri/
```

## Estrutura do projeto

```
.
├── README.md
└── app/
    ├── package.json              # Manifest + scripts do frontend
    ├── scripts/
    │   └── fetch-stockfish.mjs   # Download do sidecar Stockfish por target triple
    ├── index.html
    ├── vite.config.ts            # Porta 1420 estrita, HMR 1421
    ├── vitest.config.ts
    ├── src/                      # FRONTEND (React + TS)
    │   ├── App.tsx               # Alterna home <-> revisão
    │   ├── types.ts              # Tipos compartilhados (EngineTier, ReviewResult, ...)
    │   ├── index.css             # Tailwind + tokens de tema (dark/light)
    │   ├── components/           # Componentes de UI
    │   ├── data/eco.json         # ~500 aberturas ECO (carregado sob demanda)
    │   └── lib/                  # Lógica de negócio + testes
    │       ├── analyze.ts        # Orquestra a revisão (buildReview + analyzeGame)
    │       ├── uci.ts            # Parsers do protocolo UCI
    │       ├── scoring.ts        # cp→win%, classificação, acurácia
    │       ├── eco.ts            # Busca de abertura offline
    │       ├── pgn.ts            # Parse/validação de PGN
    │       ├── backend.ts        # Serviços Context injetáveis
    │       ├── tauri-backend.ts  # Layers de produção
    │       ├── effect/           # Erros, schemas, métricas, runtime da UI
    │       ├── review-session.ts # Sessão scoped e tarefas de análise
    │       ├── engine.ts         # Probe scoped do motor
    │       ├── engine-port.ts    # EnginePort sobre o processo Tauri
    │       ├── cache.ts          # Cache de posições (SQLite via Rust)
    │       ├── games.ts          # CRUD de partidas revisadas
    │       ├── system.ts         # Recursos do sistema + tamanho do Hash
    │       └── *.test.ts
    └── src-tauri/                # BACKEND (Rust / Tauri)
        ├── Cargo.toml
        ├── tauri.conf.json       # Janela 1180x800, sidecar, ícones
        ├── binaries/             # gitignored; fetch-stockfish.mjs coloca o binário aqui
        ├── tests/engine_handshake.rs
        └── src/
            ├── lib.rs            # Builder Tauri: plugins, DB, comandos
            ├── engine.rs         # Spawn/gerência do Stockfish; I/O UCI
            ├── db.rs             # SQLite: position_cache + games
            └── system.rs         # Núcleos físicos + RAM
```

## Arquitetura

- **Tauri 2 (núcleo Rust + webview)**: toda a análise roda no dispositivo, sem nuvem.
- **APIs Effect-first**: análise, comandos da engine, cache, histórico e recursos do sistema retornam `Effect<Success, Error, Requirements>`. Erros esperados têm tags; interrupção é um sinal de controle. Promises aparecem na borda React e nos adapters de APIs externas.
- **`EnginePort` injetável** (`src/lib/analyze.ts`): `send` retorna Effect; `onLine` e `onExit` são assinaturas locais com cleanup. Os pipelines continuam testados com engine falsa. `ask` usa Deferred e scopes, preservando respostas síncronas sem filas por linha UCI.
- **Serviços e Layers**: `backend.ts` declara `Engine`, `PositionCache`, `GamesRepository` e `SystemResources`; `tauri-backend.ts` compõe os adapters reais. A sessão injeta essas capacidades e o store, sem importar Tauri.
- **Lifecycle estruturado**: a sessão possui as fibers e os recursos em um scope; uma fila deslizante de capacidade 1 conserva a navegação mais recente. A busca anterior é interrompida e seu cleanup termina antes da próxima. Um semáforo compartilhado também serializa sessões e o teste da engine nas configurações.
- **Schema nas fronteiras**: validação de payloads IPC, preferências e JSON persistido. Cache corrompido vira miss; falha de I/O continua fatal para a análise; revisões antigas válidas são normalizadas. Logs e métricas são locais, sem exportador de telemetria.
- **Núcleo puro e sem efeitos colaterais**: `uci.ts`, `scoring.ts`, `eco.ts` e `buildReview` são funções puras; toda I/O (motor, cache, DB) é isolada e injetada.
- **UCI em Rust**: `engine.rs` faz spawn do sidecar via `tauri-plugin-shell`, escreve em stdin via canal `mpsc` e emite eventos `engine://line`/`engine://exit`. Compacta linhas `info` intermediárias antes do IPC. Um `EngineState` garante uma única instância viva.
- **SQLite duplo papel** (`db.rs`): cache identificado por FEN, modo, orçamento e MultiPV, com consultas em lote e flush incremental; histórico de revisões, com upsert por parâmetros de análise. Conexão única sob `Mutex`, em `engineroom.db` dentro de `app_data_dir`.
- **Tema via indireção de CSS vars** (`index.css`): o tema ativo é aplicado antes da pintura por um script inline em `index.html` (lê `localStorage`), evitando _flash_.
- **PGN como fonte única de verdade**: metadados (Elo, evento) são re-parseados do PGN ao reabrir, sem duplicação.

## Configurações do usuário

Persistidas em `localStorage` na chave `engineroom.settings.v1`:

- **`theme`**: `"dark"` (padrão) ou `"light"`.

## Licença

Sem licença definida no momento.
