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

Frontend tests run under Vitest 4 (pure logic and injected Effect I/O — no component tests). Effect's TestContext/TestClock work directly with the existing runner; do not install `@effect/vitest` 0.30 (it requires Vitest 3). To run one module: `bun run test src/lib/<module>.test.ts`.

## Gotchas

- **`tauri dev` starts Vite itself** via `beforeDevCommand: "bun run dev"`, strict port 1420. Don't run a separate Vite dev server alongside it.
- **Test files have a separate TS config.** `bun run typecheck` checks both the app and `tsconfig.tests.json`. Vitest transpiles tests without checking their types; keep both compiler passes.
- **Vitest runs in the `node` environment, not jsdom.** Existing tests are pure logic over `chess.js`; don't reach for DOM APIs.
- **The position cache key is `(fen, mode, depth, multipv)`**, not `(fen, depth, multipv)` as the root README says. `mode` is `"depth"` (`go depth N`) or `"time"` (`go movetime N`); the same numeric value means different things across modes — never collide them.
- **The engine process is a singleton.** `EngineState(Mutex<Option<EngineHandle>>)` in `src-tauri/src/engine.rs`; a second `engine_spawn` errors with `"A engine já está em execução."`.
- **The sidecar is referenced by basename.** `app.shell().sidecar("stockfish")` — not `"binaries/stockfish"`. Tauri resolves the platform binary from `binaries/stockfish-<triple>` automatically.
- **DB schema migrations run on every startup.** `open_file` in `src-tauri/src/db.rs` calls `migrate()` unconditionally. To add a column, write a new idempotent `migrate_*` helper gated on `PRAGMA table_info` (see `migrate_position_cache_mode` for the pattern). SQLite file: `engineroom.db` in Tauri's `app_data_dir`.
- **Theme is applied pre-paint** by an inline script in `app/index.html` that reads `localStorage["engineroom.settings.v1"]` before React mounts. Don't move theme init into a React effect — it will flash.

## Architectural invariants (don't break)

- **`EnginePort` is the test seam.** `send` and cache methods return Effects; `onLine`/`onExit` register local callbacks. `analyzeGame` returns Effect and accepts the injected port. Keep fake-engine tests; never hardcode Tauri. Old Promise-shaped regression fixtures are adapted only under `__tests__`.
- **The review decomposes into store + session + glue.** State/transitions live in `review-store.ts`; scoped orchestration lives in `review-session.ts`. `backend.ts` declares Context services (Engine, PositionCache, GamesRepository, SystemResources), composed as production Layers in `tauri-backend.ts` or fake Layers in tests. `use-review.ts` and `effect/ui-runtime.ts` are UI/runtime glue: do not move I/O into React.
- **Resource ownership is scoped.** Acquire the production port inside a Scope; never run its acquisition in an unmanaged `runPromise`. The shared permit covers startup through completed teardown, including Settings probes. A cancelled/failed live search discards the process before reuse to prevent stale `bestmove` responses.
- **Use Effect at I/O boundaries, ordinary functions in the pure core.** Execute Effects only at the UI/test boundary. Expected errors are tagged; cancellation uses fiber interruption. Preserve critical cache errors, best-effort sizing/saves and legacy Schema normalization.
- **Pure core vs. injected I/O.** `lib/uci.ts`, `lib/scoring.ts`, `lib/eco.ts`, and `buildReview` are side-effect-free. Engine, cache, and DB are always injected — keep them that way.
- **PGN is the single source of truth** for game metadata (Elo, event, result). Don't duplicate into the DB or settings.

## Style

- **Biome** (`bun run lint`): single quotes, no semicolons, trailing commas, 2-space indent, 80 cols.
- **TypeScript**: strict, `noUnusedLocals`, `noUnusedParameters`. `bun run build` runs `tsc` before `vite build`.
- **Rust**: edition 2021; release profile uses LTO + `panic = "abort"` (see `Cargo.toml`).

## Tauri IPC surface

Registered in `app/src-tauri/src/lib.rs`: `cache_get`, `cache_put`, `cache_get_bulk`, `cache_put_many`, `cache_clear`, `games_save`, `games_list`, `games_get`, `games_delete`, `games_clear`, `storage_stats`, `engine_spawn`, `engine_send`, `engine_stop`, `system_resources`. `cache_get_bulk`/`cache_put_many` batch a whole game in one IPC (prefetch all hits; flush writes in one transaction) — the analysis loop no longer does per-position cache round-trips. The engine emits one `engine://line` Tauri event per stdout line; the frontend subscribes via `EnginePort` (`src/lib/engine-port.ts`).
