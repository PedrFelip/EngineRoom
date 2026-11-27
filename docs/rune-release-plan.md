# Empacotamento Rune

O EngineRoom usa `cargo-packager` para gerar AppImage e NSIS com Stockfish
ao lado do executável. Os scripts de empacotamento verificam o inventário e
executam testes dos pacotes. O workflow de release só publica após checks e
builds no commit imutável da tag.

Consulte [releases.md](releases.md) para as dependências, comandos e regras de
publicação.
