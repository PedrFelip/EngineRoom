# EngineRoom — avisos de licença

Copyright (c) 2026 Pedro Felipe.

Esta versão do EngineRoom é distribuída sob **GPL-3.0-or-later**. O texto da
GPL versão 3 está em `LICENSE`. A versão anterior e seus avisos MIT permanecem
registrados em `licenses/LEGACY-MIT.txt`.

Bibliotecas de xadrez vinculadas ao backend:

| Biblioteca | Versão | Licença | Código-fonte |
| --- | --- | --- | --- |
| shakmaty | 0.30.1 | GPL-3.0-or-later | https://github.com/niklasf/shakmaty |
| pgn-reader | 0.29.0 | GPL-3.0-or-later | https://github.com/niklasf/shakmaty |

As versões exatas e as demais dependências do backend estão em
`app/src-tauri/Cargo.lock`; as do frontend estão em `app/bun.lock`.
Dependências conservam suas próprias licenças e avisos.

O Stockfish 18 é executado como sidecar separado. Seu código-fonte está em
https://github.com/official-stockfish/Stockfish/tree/sf_18, e o release utilizado
pelo downloader está em https://github.com/official-stockfish/Stockfish/releases/tag/sf_18.
Os avisos e a licença do Stockfish estão no repositório original.

O dataset ECO existente mantém sua origem em `lichess/chess-openings`; esta
migração incorpora o mesmo arquivo no backend e não substitui seus dados.
