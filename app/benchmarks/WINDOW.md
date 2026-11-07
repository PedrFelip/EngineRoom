# Janela experimental de continuidade

`continuity_window.py` executa simultaneamente as três posições consecutivas de abertura de `games.pgn` usadas pelo experimento anterior. Cada posição distinta tem um SearchWork lógico e um processo exclusivo; FENs idênticas são deduplicadas. É uma única janela de até três posições, não uma revisão de partida inteira. Todos os processos começam juntos, garantindo progresso da triagem. Não implementa seleção adaptativa Rust, quotas, reconciliação global, cache ou persistência.

Em cada `go infinite`, MultiPV permanece fixo no valor de triagem do perfil. No prazo de triagem, salva-se uma cópia do último conjunto coerente recebido até o prazo, usando `Engine.get`, o timestamp do leitor e `coherent` da base. A decisão experimental usa as baselines dos vizinhos disponíveis (um no extremo, dois no meio): promover se qualquer diferença absoluta, normalizada para brancas, for de pelo menos 100 centipawns; mate é representado por ±10000. Score ausente não promove. Isso é apenas uma regra local de demonstração, não a classificação do produto.

A busca continua enquanto espera vizinhos por no máximo 150 ms (limitados ao orçamento adicional). Se promovida em execução, mantém o mesmo ID de `go`, até o total nominal `triageMs + mediumMs` ou `triageMs + highMs`; se descartada, recebe `stop`. Ao esgotar a espera ela para; uma promoção tardia inicia outro `go` apenas com o orçamento nominal restante, descontando o tempo de busca anterior, inclusive espera e término. `stop` e `bestmove` podem ultrapassar o prazo; eventos registram tempo e limite. Falha encerra todos os processos e aguarda seu término. Não há limite rígido de CPU-segundos nem medição de CPU agregada: Threads e Hash configurados são limitados globalmente, mas Hash não inclui NNUE, RSS ou overhead. `peak_rss_kb` é o pico individual amostrado, não o agregado. Eventos mostram `go`, baseline, promoção/descarte, `stop` e `bestmove`, com IDs e espera; resultados registram duração e contagens. O relógio e o escalonamento podem gerar overshoot.

```sh
python -m unittest discover -s app/benchmarks -p 'test_continuity*.py'
python app/benchmarks/continuity_window.py --profile fast --target medium --threads-total 3 --hash-total-mb 48 --output /tmp/engineroom-window.json
```

A amostra real isolada em `/tmp/engineroom-window.json` usou Stockfish sidecar, três processos, uma thread e 16 MiB Hash cada, MultiPV 3, triagem 180 ms e total nominal promovido 680 ms. Durou 516,5 ms de parede; houve zero promoções, três descartes e zero reinícios. Esperas observadas para decisão foram 0, 10,4 e 13,8 ms. Não se observou promoção real neste corpus de abertura; o teste determinístico usa scores controlados para cobrir promoção e descarte e verificar o mesmo ID de `go`. A amostra não mede CPU diretamente, mas sua duração e limite de três threads a colocam bem abaixo de aproximadamente 60 segundos de CPU. Não houve acesso ao banco.

Uma execução, três posições e nenhum alvo promovido não demonstram benefício, qualidade, throughput ou comportamento em partidas reais. Comparação válida requer corpus tático e maior, repetições, medição de CPU e memória agregadas, mesmos alvos e cobertura, além de referência independente. MultiPV fixo difere do refinamento atual; não compare apenas o tempo do segundo `go` nem infira qualidade da profundidade. O fixture controlado não substitui um caso real de promoção.

## Caso tático adicional

O corpus versionado `continuity-tactical.json`, gerado com chess.js, contém as posições consecutivas após `1. f3 e5` e `2. g4`. Para reproduzir:

```sh
python app/benchmarks/continuity_window.py --threads-total 3 --hash-total-mb 48 --corpus app/benchmarks/continuity-tactical.json --output app/benchmarks/results/continuity-window-tactical.json
```

A execução registrada teve duas promoções, zero reinícios e dois IDs de `go` preservados entre baseline e término. Usou dois processos com uma thread e 24 MiB Hash cada (duas threads efetivas sob teto de três). O tempo total foi 1.299 ms nesta execução; não constitui comparação de desempenho. O critério detecta uma diferença de avaliação entre vizinhos, sem atribuir corretamente por si só responsabilidade ou classificação de lance; não é substituto do adaptive Rust. A janela não desliza pela partida.

O checkpoint drena a fila já carimbada pelo leitor antes de congelar a baseline. Depois de enviar `stop`, uma decisão tardia só pode produzir reinício explícito, nunca promoção contínua. Há regressão para essa transição.
