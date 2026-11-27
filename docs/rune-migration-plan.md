# Migração para Rune

O workspace Rust/Rune é o aplicativo desktop do EngineRoom e está na raiz do
repositório. O código Tauri foi removido. A distribuição usa AppImage Linux e
NSIS Windows, com instalação manual.

O aplicativo usa `<dados>/engineroom/` para histórico, cache e preferências.
Ele importa dados da prévia Rune em `<dados>/engineroom-experimental/` na
primeira abertura e conserva os arquivos originais. A migração da versão Tauri
não é um requisito.

O CI executa formatação, Clippy, testes do workspace, testes reais de
Stockfish e verificações headless e responsivas. A publicação exige também os
dois instaladores e o smoke test Linux. O fluxo está descrito em
[releases.md](releases.md).
