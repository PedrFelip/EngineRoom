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
- **Persistência local (SQLite)**: cache de posições por `(fen, depth, multipv)` e histórico de partidas revisadas com reabertura instantânea, reanálise e exclusão.
- **Autoconfiguração do motor**: ajusta `Threads` (núcleos físicos) e `Hash` (~20% da RAM, entre 512 MB e 4 GB) automaticamente.
- **Engine customizada**: use o Stockfish embarcado ou aponte para um binário próprio nas configurações, com botão de teste.
- **Tema claro/escuro** aplicado antes da pintura para evitar *flash*.

## Stack

| Camada | Tecnologias |
| --- | --- |
| App | Tauri **2** |
| Backend | Rust (edição 2021), `tokio`, `rusqlite` (SQLite *bundled*), `sysinfo`, `serde` |
| Frontend | React **19**, TypeScript **6**, Vite **8**, Tailwind CSS **4** |
| Xadrez | `chess.js` (PGN/FEN), `chessground` (tabuleiro) |
| Motor | Stockfish **18** (sidecar baixado em build/dev) |
| Testes | Vitest (frontend), `cargo test` (backend) |
| Gestor de pacotes | **Bun** |

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

| Script | Comando | Descrição |
| --- | --- | --- |
| `dev` | `vite` | Dev server do frontend (porta 1420) |
| `build` | `tsc && vite build` | Type-check + build de produção |
| `preview` | `vite preview` | Pré-visualiza o build |
| `tauri` | `tauri` | Pass-through para a CLI do Tauri |
| `test` | `vitest run` | Roda os testes do frontend uma vez |
| `test:watch` | `vitest` | Testes do frontend em modo *watch* |
| `typecheck` | `tsc --noEmit` | Somente type-check |

## Testes

```bash
# Frontend (7 arquivos em src/lib/*.test.ts)
bun run test

# Backend (testes unitários em src/db.rs, src/system.rs + teste de integração)
cargo test                # dentro de app/src-tauri/
```

> O teste de integração `engine_handshake.rs` faz um handshake UCI real com o Stockfish e **pula graciosamente** (sem falhar) caso o binário sidecar não esteja presente.

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
    │   ├── components/           # Componentes de UI (13)
    │   ├── data/eco.json         # ~500 aberturas ECO (carregado sob demanda)
    │   └── lib/                  # Lógica de negócio + testes (20 módulos)
    │       ├── analyze.ts        # Orquestra a revisão (buildReview + analyzeGame)
    │       ├── uci.ts            # Parsers do protocolo UCI
    │       ├── scoring.ts        # cp→win%, classificação, acurácia
    │       ├── eco.ts            # Busca de abertura offline
    │       ├── pgn.ts            # Parse/validação de PGN
    │       ├── engine.ts         # Wrappers IPC do motor
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
- **`EnginePort` injetável** (`src/lib/analyze.ts`): `analyzeGame` depende de uma interface `send`/`onLine`, não do processo concreto. Isso permite testar todo o pipeline (win%, classificação, acurácia, multipv, cache) com um `fakePort`, sem o Stockfish. `createTauriEnginePort` é o adaptador de produção.
- **Núcleo puro e sem efeitos colaterais**: `uci.ts`, `scoring.ts`, `eco.ts` e `buildReview` são funções puras; toda I/O (motor, cache, DB) é isolada e injetada.
- **UCI em Rust**: `engine.rs` faz spawn do sidecar (via `tauri-plugin-shell`) ou de um caminho customizado (`tokio::process`), escreve em stdin via canal `mpsc` e reemite cada linha de stdout como evento Tauri `engine://line`. Um `EngineState` (`Mutex<Option<EngineHandle>>`) garante uma única instância viva.
- **SQLite duplo papel** (`db.rs`): tabela `position_cache` (chave exata `fen + depth + multipv`) e tabela `games` (`UNIQUE(pgn, depth, multipv)` — reanalisar com mesmos parâmetros substitui a entrada). Conexão única sob `Mutex`, em `engineroom.db` dentro de `app_data_dir`.
- **Tema via indireção de CSS vars** (`index.css`): o tema ativo é aplicado antes da pintura por um script inline em `index.html` (lê `localStorage`), evitando *flash*.
- **PGN como fonte única de verdade**: metadados (Elo, evento) são re-parseados do PGN ao reabrir, sem duplicação.

## Configurações do usuário

Persistidas em `localStorage` na chave `engineroom.settings.v1`:

- **`theme`**: `"dark"` (padrão) ou `"light"`.
- **`enginePath`**: vazio = usar o Stockfish embarcado; ou caminho absoluto para um binário próprio.

## Licença

Sem licença definida no momento.
