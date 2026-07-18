# AGENTS.md

Tauri 2 desktop app (Rust + React 19) that reviews chess games locally with Stockfish 18. The root `README.md` has the full feature/architecture overview — this file only captures what isn't obvious from the repo.

## Working directory

- Run frontend and Tauri commands from `app/`; run Rust commands from `app/src-tauri/`. The repo root holds only READMEs.
- `app/README.md` is leftover Tauri template boilerplate — ignore it. The canonical README is at the repo root.

## First-run setup (from `app/`)

```bash
bun install
node scripts/fetch-stockfish.mjs   # idempotent; downloads Stockfish 18 for the host triple
```

The sidecar lives at `app/src-tauri/binaries/stockfish-<triple>` (gitignored). Without it, analyzing a game fails at `engine_spawn`, and the Rust integration test in `src-tauri/tests/engine_handshake.rs` silently skips (does not fail).

## Before claiming work is done

From `app/`:
```bash
bun run lint && bun run typecheck && bun run test
```
From `app/src-tauri/`:
```bash
cargo test
```
There is no CI and no pre-commit hook — verification is manual.

Single frontend test file: `bun run test src/lib/scoring.test.ts`.

## Gotchas

- **`tauri dev` starts Vite itself** via `beforeDevCommand: "bun run dev"`, strict port 1420. Don't run a separate Vite dev server alongside it.
- **Test files are excluded from `tsconfig.json`** (`exclude: ["src/**/*.test.ts", "src/**/*.test.tsx"]`). `bun run typecheck` does NOT typecheck tests — test-only type errors surface only under Vitest.
- **Vitest runs in the `node` environment, not jsdom.** Existing tests are pure logic over `chess.js`; don't reach for DOM APIs.
- **The position cache key is `(fen, mode, depth, multipv)`**, not `(fen, depth, multipv)` as the root README says. `mode` is `"depth"` (`go depth N`) or `"time"` (`go movetime N`); the same numeric value means different things across modes — never collide them.
- **The engine process is a singleton.** `EngineState(Mutex<Option<EngineHandle>>)` in `src-tauri/src/engine.rs`; a second `engine_spawn` errors with `"A engine já está em execução."`.
- **The sidecar is referenced by basename.** `app.shell().sidecar("stockfish")` — not `"binaries/stockfish"`. Tauri resolves the platform binary from `binaries/stockfish-<triple>` automatically.
- **DB schema migrations run on every startup.** `open_file` in `src-tauri/src/db.rs` calls `migrate()` unconditionally. To add a column, write a new idempotent `migrate_*` helper gated on `PRAGMA table_info` (see `migrate_position_cache_mode` for the pattern). SQLite file: `engineroom.db` in Tauri's `app_data_dir`.
- **Theme is applied pre-paint** by an inline script in `app/index.html` that reads `localStorage["engineroom.settings.v1"]` before React mounts. Don't move theme init into a React effect — it will flash.

## Architectural invariants (don't break)

- **`EnginePort` is the test seam.** `analyzeGame` in `src/lib/analyze.ts` takes `port: { send, onLine }`. The whole pipeline (win%, classification, accuracy, multipv, cache) is tested with a fake port — never with the real Stockfish. Extend analysis through this seam; don't hardcode the Tauri adapter (`createTauriEnginePort`).
- **Pure core vs. injected I/O.** `lib/uci.ts`, `lib/scoring.ts`, `lib/eco.ts`, and `buildReview` are side-effect-free. Engine, cache, and DB are always injected — keep them that way.
- **PGN is the single source of truth** for game metadata (Elo, event, result). Don't duplicate into the DB or settings.

## Style

- **Biome** (`bun run lint`): single quotes, no semicolons, trailing commas, 2-space indent, 80 cols.
- **TypeScript**: strict, `noUnusedLocals`, `noUnusedParameters`. `bun run build` runs `tsc` before `vite build`.
- **Rust**: edition 2021; release profile uses LTO + `panic = "abort"` (see `Cargo.toml`).

## Tauri IPC surface

Registered in `app/src-tauri/src/lib.rs`: `cache_get`, `cache_put`, `games_save`, `games_list`, `games_get`, `games_delete`, `engine_spawn`, `engine_send`, `engine_stop`, `system_resources`. The engine emits one `engine://line` Tauri event per stdout line; the frontend subscribes via `EnginePort` (`src/lib/engine-port.ts`).
