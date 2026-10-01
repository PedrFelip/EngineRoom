# AGENTS.md

Tauri 2 desktop app (Rust + React 19) that reviews chess games locally with Stockfish 18. The root `README.md` is the canonical feature/architecture overview; `app/README.md` is template boilerplate.

## Working directory and setup

- Run frontend/Tauri commands from `app/`; Rust commands from `app/src-tauri/`.
- First setup: `bun install`, then `node scripts/fetch-stockfish.mjs` from `app/`.
- The gitignored sidecar is `src-tauri/binaries/stockfish-<triple>`. The real Rust session test fails if it cannot spawn the bundled sidecar; do not report it as verified when only the old handshake test skipped.
- `tauri dev` starts Vite through `beforeDevCommand`; do not start a second Vite on strict port 1420.

## Required verification

From `app/`: `bun run lint && bun run typecheck && bun run test`.
From `app/src-tauri/`: `cargo test`.
There is no CI/pre-commit hook. `typecheck` checks the app and `tsconfig.tests.json`; Vitest transpilation alone does not type-check tests.
Vitest 4 uses `node`, not jsdom. Use Effect TestContext/TestClock directly; do not install `@effect/vitest` 0.30 (requires Vitest 3).

## Architecture and resource ownership

- **Rust owns analysis.** `src-tauri/src/review/` contains the pure chess/scoring/adaptive core, UCI transport, injected pipeline, repository and window-owned session task. Manual, adaptive, live, playback, cache and persistence decisions belong here.
- **EngineFactory/EnginePort and Repository are Rust test seams.** Keep Rust fake-engine tests. Do not hardcode Tauri in the pure core or the pipeline. The shell and SQLite adapters are production implementations.
- **The engine lease is exclusive.** `ReviewSessions` shares a Rust semaphore between review sessions and Settings probes. Automatic reviews use a bounded mixed queue of Stockfish workers under one shared permit; manual reviews use one process. Automatic workers claim four consecutive triage positions, with one preferring ready hard refinements. Preserve profile budgets and final soft quotas; publish/cache in completion order. Divide the total Threads/Hash budgets across workers. An owner holds its permit through completed teardown of every process; never stop another owner's process. After interruption/failure of a live search, kill and await termination before reuse.
- **Sidecar framing is explicit.** Use `app.shell().sidecar("stockfish")` (basename only), with `set_raw_out(true)` before spawn. The frontend receives structured Channels, never raw UCI stdout. The shared stdout helper preserves partial chunks.
- **Latest navigation wins.** Session intents carry monotonic request IDs. Updates/cancellations invalidate older live searches, and the session finishes their cleanup before starting the latest. Frontend events carry session ID, sequence and live request ID; reject stale events before applying them to the store.
- **Close is acknowledged after cleanup.** `review_session_close` is idempotent. Window destruction signals cancellation; app exit waits for session cleanup. Scoped frontend acquisition must finish before honoring interruption, then close the acquired session. Track outstanding blocking DB operations and drain them at session close.
- **UI state remains separate.** `review-store.ts` owns navigation/variation transitions. `review-session.ts` and `effect/ui-runtime.ts` are scoped IPC/store glue over the injected `AnalysisSessions` Context service. Keep I/O out of React. UI methods enqueue intent synchronously; asynchronous IPC runs inside their scope.
- **Use Effect at frontend I/O boundaries**, tagged errors and Schema validation for IPC/settings. Execute Effects at the UI/test boundary. Cancellation must not appear as an analysis error. Critical cache errors propagate; resource sizing and history saves are best-effort.
- **PGN is the metadata source.** Rust revalidates the mainline, respects FEN setup, derives metadata for saving and normalizes old review JSON when reopening. Keep PGN Elo/event out of new DB/settings fields. Existing historical summary fields are retained for compatibility. `chess.js` remains for import preview and board/variation interactions.

## Cache and persistence

- The current DB PK is `(fen, reached_depth, multipv)`, with `source_mode` and `source_value`. Depth requests accept reached depth >= requested depth (including time entries); time requests only accept time entries with budget >= requested budget. Both require sufficient MultiPV. Preserve the current covering SQL; numeric time/depth budgets are not interchangeable.
- Cache prefetch uses bulk queries; writes flush every eight completed positions and at the end. Corrupt JSON becomes a miss; actual SQLite I/O failure remains fatal. Failure flush is best-effort and must preserve the original error.
- The Rust session publishes completion before best-effort history save and owns that save through teardown. Saved JSON and current normalization remain compatible; this migration needs no schema change.
- `db::open_file` applies migrations every startup. Future migrations must be idempotent and gated on `PRAGMA table_info`, following existing helpers. SQLite is `engineroom.db` in `app_data_dir`.

## UI/style gotchas

- Theme is applied pre-paint in `app/index.html` using `localStorage["engineroom.settings.v1"]`; never move it into a React effect.
- Biome: single quotes, no semicolons, trailing commas, two spaces, 80 columns.
- TypeScript is strict with unused locals/parameters rejected. `bun run build` runs tsc before Vite.
- Rust edition 2021; release has LTO, opt-level 3 and `panic = "abort"`. Use `cargo fmt`.
- Distribution is GPL-3.0-or-later, as approved for linked shakmaty/pgn-reader dependencies. Preserve `LICENSE` and third-party notices in bundled resources.

## Tauri IPC

Session commands: `review_session_open`, `review_session_analyze_position`, `review_session_cancel_live`, `review_session_close`, `engine_probe`, `games_get_review_config`.
Existing cache/history/storage/system commands remain registered for administration and compatibility. Raw engine spawn/send/stop commands are no longer public. UCI benchmark commands are restricted to `--bench-uci-ipc` and describe the historical transport, not the new session.
`bun run bench:rust` measures the Rust fake-engine pipeline in release mode. It excludes Stockfish, IPC and rendering; do not present those figures as end-to-end game speedups.
