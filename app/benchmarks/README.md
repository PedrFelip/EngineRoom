# Benchmark comparativo da pipeline

Execute de `app/`:

```sh
bun run bench:compare
```

O runner compila o Rust em release, prepara o bundle TS para Node e executa
três rodadas sequenciais, alternando TS/Rust. Resultados completos, logs,
amostras, mediana, p95 e pico RSS ficam em `/tmp/engineroom-benchmark/`.
Use `--output /caminho` e `--rounds N` para mudar essas opções.
Requer Linux, Python 3, Bun, Node, Cargo e o sidecar baixado pelo setup.
`BENCH_STOCKFISH` pode apontar para outro binário; o padrão é o sidecar Linux x64.

São 41 cenários: buscas depth/time e MultiPV 1/3, partidas completas de
diferentes tamanhos, mate, promoção/FEN, cache frio/quente/misto, adaptativo
fast/deep e cancelamento. Oito cenários usam Stockfish 18 real com uma thread,
Hash de 16 MiB e depth 8 ou 20 ms por posição, incluindo spawn e teardown.
Os demais usam UCI falso com 32 atualizações por linha MultiPV e scores
sintéticos para isolar o custo da pipeline. O cache injetado contabiliza
consultas/escritas, mantém hits estáticos e não executa SQLite.

TS usa a referência congelada, sem importá-la na aplicação. A engine falsa
roda no Bun; o motor real roda no Node, porque o Stockfish encerrou antes do
handshake no transporte por pipes do Bun observado nesta máquina. Essa
diferença de runtime é identificada nos resultados. O Rust usa a pipeline
injetada e um transporte headless por pipes para o Stockfish real.

Cada rodada falsa tem três aquecimentos e quinze amostras; cada rodada real,
um aquecimento e cinco amostras. Tempos são normalizados pelo tamanho do lote
(cinco partidas, 300 buscas ou 100 cancelamentos). O p95 é de lotes
normalizados, não da latência individual dos eventos. Compilação, geração
de corpus e escrita de relatório não entram nas amostras.

O runner compara resultados completos da engine falsa com tolerância
absoluta de `1e-9`, classificações exatas e contadores de trabalho. Para o
motor real, compara mainline, posições e trabalho; scores e profundidades
podem variar, sobretudo em buscas por tempo. Falhas de FEN inicial na
referência TS são registradas como incompatibilidades, sem inventar um
speedup. Outras falhas ou divergências fazem o comando terminar com erro.

Não mede IPC Tauri, renderização ou I/O SQLite, nem representa o tempo
de ponta a ponta do app. Também não isola o efeito da linguagem dos efeitos
de runtime, transporte e arquitetura. O RSS inclui harness/resultados e não
soma a memória dos processos; o heap retido Bun é apenas diagnóstico.

## Corpus

[games.pgn](games.pgn) contém dez partidas reais, selecionadas da coleção
[tonymorris/immortalgames](https://github.com/tonymorris/immortalgames/blob/master/immortal_games.pgn),
baixada em 2026-09-29. Foram preservados os lances e metadados, removendo
comentários e análises. Todos os PGNs foram validados com `chess.js`.
O corpus vai de 34 a 129 meios-lances e inclui promoção, mate, sacrifícios,
empate e finais. Os scores da carga falsa são sintéticos, não avaliações
dessas partidas.

Para regenerar a carga compartilhada de Rust e TS:

```sh
bun scripts/generate-benchmark-fixtures.mjs
```

O JSON fica em `src-tauri/src/review/fixtures/benchmark.json`. Hashes do
corpus e da carga, versões de ferramentas e revisão Git são registrados
em cada execução. `--summarize-only --output /caminho` refaz a síntese a
partir de JSONs existentes, sem repetir as medições.
