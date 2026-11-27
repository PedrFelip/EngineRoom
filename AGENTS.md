# AGENTS.md

EngineRoom é um aplicativo desktop Rust/Rune para revisar partidas de xadrez
localmente com Stockfish 19. O README raiz descreve o produto e os comandos.

## Desenvolvimento e verificação

Execute os comandos na raiz. Baixe o motor com
`node scripts/fetch-stockfish.mjs`, depois use `cargo run --locked`.

Antes de concluir alterações, execute `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`,
`cargo test --workspace --locked`, os testes reais de Stockfish com
`ENGINE_ROOM_STOCKFISH` e `cargo test --locked -p engineroom-rune real_stockfish -- --ignored --test-threads=1`,
`./scripts/check-headless.sh` e `./scripts/check-responsive.sh`.
Para automação de release execute `python -m unittest discover -s .github/scripts -p 'test_*.py'`
e `bun install --frozen-lockfile && bun run test` em `.github/scripts/`.

## Arquitetura

- O domínio, os perfis e a pontuação ficam em `crates/engineroom-domain`.
  Pipeline, portas, sessões e controle de recursos ficam em
  `crates/engineroom-application`. Use engines falsas nos testes.
- `crates/engineroom-uci` gerencia Stockfish e seu ciclo de vida.
  O lease é exclusivo e só é liberado após encerrar e aguardar os processos.
  A navegação mais recente invalida buscas ao vivo anteriores.
- `crates/engineroom-sqlite` mantém cache, histórico e migrações idempotentes.
  Preserve consultas de cobertura, tratamento de corrupção e erros de I/O.
- O aplicativo Rune fica em `crates/engineroom-rune/src`, com manifesto na raiz.
  I/O e tarefas pertencem às portas e à bridge, não aos componentes visuais.
  Chess.com usa HTTP nativo com limites e cancelamento no backend.
- O sidecar baixado fica em `binaries/stockfish-<triple>`. Os pacotes o
  colocam ao lado do executável. Não inclua binários baixados no Git.
- Preserve `LICENSE`, `RUNE-LICENSE.txt` e avisos de terceiros nos pacotes.

## Releases

Use PRs Conventional Commit para `develop` e squash merge. Promova `develop`
para `main` com merge commit. O Release Please cria um draft e a publicação
exige checks e instaladores AppImage e NSIS verificados no commit da tag.
Mantenha o gate terminal `CI passed`. A retomada manual usa uma tag existente.
Detalhes em `docs/releases.md`.
