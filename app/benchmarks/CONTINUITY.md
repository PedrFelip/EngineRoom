# Experimento isolado de continuidade UCI

Este protótipo executa **uma posição por vez**, sem janela concorrente de duas buscas e sem decisão adaptativa local. Não mede throughput, contenção, prioridade, cache, persistência ou a revisão completa. Uma arquitetura de buscas abertas durante a seleção de refinamentos precisaria de outro experimento com janela limitada, políticas de seleção e cancelamento, orçamento global e propriedade dos processos. Não existe integração de produção aqui.

## Reprodução

Na raiz, com Python 3.11+ e sidecar instalado:

```sh
python -m unittest discover -s app/benchmarks -p 'test_continuity_experiment.py'
python app/benchmarks/continuity_experiment.py --repetitions 2 --threads 1 --hash-mb 16 --profile fast --target medium --output app/benchmarks/results/continuity-fast-medium.json
```

Em outra plataforma informe `--binary /caminho/do/stockfish`. Recursos são por processo. O script lê `profiles.json` a cada execução. As FENs são a posição inicial e os dois primeiros meios-lances da primeira partida de `games.pgn` (Larsen–Spassky). A ordem gira por posição e é invertida em repetições ímpares; `runs` contém a ordem efetiva. Cada caso começa com processo novo. O corpus pequeno de abertura não representa partidas.

- `equal_multipv`: continuous inicia um único `go infinite`, guarda checkpoint no orçamento de triagem e para no orçamento total; warm para após triagem, aguarda `bestmove` e inicia outro `go` no mesmo processo; cold encerra e inicia outro processo. Todos usam MultiPV de triagem nas duas etapas, mesmas FENs, Threads, Hash e orçamento nominal total. Cada reinício recebe o tempo total menos o de triagem; overhead é adicional.
- `current_multipv_restart_only`: warm e cold usam MultiPV de triagem e depois o do perfil para o alvo escolhido. Não há continuous equivalente: trocar MultiPV durante a busca não é modelado. Este cenário não faz o mesmo trabalho que o de MultiPV igual.

Warm conserva potencialmente a tabela de transposição (TT) e outros estados internos do Stockfish entre os dois `go`; cold não conserva esse processo. Isso não é cache de resultados do EngineRoom: não há DB nem leitura ou escrita de cache. Não há garantia de que todo estado interno seja útil ou preservado.

Cada linha é carimbada com relógio monotônico no thread leitor antes de entrar na fila. Cada linha `info` com score e PV é registrada com texto, ID do `go`, tempo de parede desde o início do `go`, depth, nodes, tempo UCI, score e PV. `nodes` é cumulativo **dentro de cada go**, portanto não some as linhas do mesmo go. O checkpoint drena a fila já recebida e copia as últimas linhas carimbadas até o prazo que formem todos os índices MultiPV na mesma profundidade, com scores sem bounds; uma emissão posterior com bound invalida a emissão exata anterior do mesmo índice e profundidade. Pode ser `null` se não houver ciclo completo; não se espera uma nova linha após o prazo. O resultado final usa o mesmo critério. Não garante simultaneidade além da profundidade reportada.

`stages[].wall_ms` inclui busca e espera de `bestmove`; `startup_ms` inclui spawn, handshake e configuração inicial; `teardown_ms` inclui kill e wait; `wall_ms` do caso inclui ambos e configuração entre buscas. O processo é finalizado com kill e wait inclusive em erro, timeout e Ctrl-C. No Linux, `peak_rss_kb_max_process` é o maior VmHWM amostrado em `/proc` dos processos Stockfish individuais, não do runner; não inclui filhos e pode perder o pico de processo muito curto. Não é soma simultânea nem medida portável; pode ser null. Binário, SHA-256, identificação UCI, perfil integral, recursos, FENs, repetição e ordem estão no JSON. Tempos de recebimento no Python incluem ruído de buffers e escalonamento.

## Amostra registrada

`results/continuity-fast-medium.json`: Stockfish 19, perfil fast, triagem 180 ms, refinamento adicional 500 ms, total nominal 680 ms, MultiPV 3 na comparação igual, 1 thread, Hash 16 MiB, três posições e duas repetições. Cada modo possui seis amostras. A amostra foi regenerada após corrigir o orçamento acumulado, a invalidação de linhas com bound e os horários de recebimento. Consulte as amostras individuais no JSON para tempos de busca, inicialização e encerramento; o custo de criar outro processo em cold deve ser separado do custo da pesquisa.

São seis observações por modo, não um teste estatístico, de qualidade das jogadas ou de desempenho da pipeline completa. Profundidade não é qualidade comparável por si só; busca, hardware e carga do sistema afetam os resultados. Não há evidência suficiente para prometer ganho ou mudar produção.

Na amostra corrigida, as médias de busca foram 680,7 ms (continuous), 680,9 ms (warm) e 681,0 ms (cold). As médias totais foram 916,3 / 915,0 / 1.162,7 ms, respectivamente, incluindo inicialização e limpeza. Como o ensaio é temporizado, igualdade de duração de busca é esperada: a diferença de custo/qualidade precisa ser avaliada pelo trabalho obtido nesse tempo, não pela latência isolada. A diferença de cold é dominada pela inicialização adicional. Estes dados não mostram superioridade de continuidade sobre reinício aquecido.
