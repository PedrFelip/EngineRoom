<div style="text-align: center;"><img src="docs/brand/engineroom-icon.svg" alt="Ícone do EngineRoom" width="80" height="80"></div>

# EngineRoom

Revise partidas de xadrez localmente com Stockfish 19. Importe um arquivo PGN,
cole uma partida ou busque partidas públicas no Chess.com. Escolha um perfil
automático ou configure a análise manual e navegue pelos lances, avaliações,
classificações e variações no tabuleiro.

O histórico e o cache de posições ficam no computador. A busca no Chess.com
exige internet. A análise da partida é local.

## Executar

Instale Rust estável, Node.js e as bibliotecas nativas de áudio, X11/Wayland e
Vulkan. Na raiz do repositório, execute:

```sh
node scripts/fetch-stockfish.mjs
cargo run --locked
```

O downloader instala Stockfish 19 em `binaries/`. O aplicativo distribuído
inclui o motor. É possível apontar `ENGINE_ROOM_STOCKFISH` para outro executável
durante o desenvolvimento.

## Verificar

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
ENGINE_ROOM_STOCKFISH="$PWD/binaries/stockfish-$(rustc -vV | sed -n 's/^host: //p')" \
  cargo test --locked -p engineroom-rune real_stockfish -- --ignored --test-threads=1
./scripts/check-headless.sh
./scripts/check-responsive.sh
python -m unittest discover -s .github/scripts -p 'test_*.py'
cd .github/scripts && bun install --frozen-lockfile && bun run test
```

## Arquitetura

O workspace Rust separa o domínio de xadrez, a aplicação e suas portas de motor
e persistência, o transporte UCI, SQLite e a interface Rune. O aplicativo desktop
fica em `crates/engineroom-rune/src`, com seu manifesto na raiz. A busca
Chess.com e a análise executam no backend nativo.

O cache considera profundidade alcançada, orçamento de tempo e número de linhas.
Sessões compartilham um acesso exclusivo ao motor e aguardam a limpeza dos
processos ao encerrar.

## Distribuição

As releases fornecem AppImage para Linux x86_64 e instalador NSIS para Windows
x86_64. A instalação e as atualizações são manuais. Veja [releases](docs/releases.md)
para o empacotamento e a automação.

## Licença

GPL-3.0-or-later. Veja [LICENSE](LICENSE) e os
[avisos de terceiros](THIRD_PARTY_NOTICES.md).
