# Avaliação e decisão de adoção de B

Estado em 2026-11-03: **manter experimental**. Uma comparação exploratória
pareada de uma partida completa e uma referência mais cara estão registradas em
`evidence/b-paired-2026-11-03/README.md`; essa amostra e suas limitações não
satisfazem o protocolo de adoção. A conclusão de protocolo de B não prova que todas as
decisões estão resolvidas ou que sua qualidade é superior. A opção legacy segue
disponível para uma nova revisão; uma execução B em andamento não é convertida
silenciosamente.

## Protocolo proposto antes da coleta

Congelar e registrar o commit, corpus e hashes de PGN, executável Stockfish e
manifestos, versões de ferramentas, sistema, CPU, memória, limites de Threads e
Hash e perfil legado. Usar as mesmas partidas completas e raiz FEN em ambas as
pipelines, incluindo partidas longas, táticas, finais, promoções e repetição.
Registrar a seleção do corpus e exclusões antes de observar os resultados; o
pequeno corpus em `app/benchmarks/games.pgn` é ponto de partida, não amostra
representativa demonstrada.

Separar cache frio e quente e registrar o estado inicial e os hits. Repetir cada
partida nas mesmas condições de máquina, alternando a ordem A/B e reportando cada
amostra, mediana e dispersão por partida. Executar sem outras revisões ou probes.
Registrar tempo até primeira cobertura, conclusão e teardown, CPU e memória
quando disponíveis, número de buscas, posições cobertas, verificações e
pendências. Medições ausentes são `null`, não zero. Reportar também tempo e
resultados com IPC, UI e persistência, separadamente do runner.

Para cada lance, comparar melhor e jogado a uma referência independente de
Stockfish com orçamento maior, mesmo contexto e configuração registrada. Uma
referência mais cara não é verdade absoluta: revisar divergências críticas e
conservar scores, linhas, profundidades e posições que as explicam. Não fundir
mate com centipawns em uma diferença numérica. Contar decisões ausentes e
unresolved separadamente; não excluí-las do denominador como se fossem acertos.
Informar diferenças pareadas de qualidade e custo e dispersão, sem atribuir à
política efeitos combinados de contexto, cache, seleção ou concorrência.

Os benchmarks existentes em `app/benchmarks/README.md` documentam baseline e
referência para a pipeline atual; a comparação exploratória com B é descrita em
`app/benchmarks/B-PAIRED.md`. Antes de rodar a comparação final,
fixar corpus, repetições, orçamentos e margens quantitativas de qualidade,
latência e custo aceitas pelo produto. Qualquer mudança posterior exige nova
coleta ou identificação explícita como análise exploratória.

## Gates operacionais para revisão da decisão

- Nenhuma perda de trabalho obrigatório, evidência incompatível aceita ou
  processo sobrevivente após cancelamento, falha ou fechamento nos testes.
- Migração idempotente e reabertura de dados existentes e B verificadas; limpeza
  de cache não pode destruir histórico salvo.
- Exercitar fallback por nova revisão, aquisição interrompida, navegação rápida,
  falha de persistência e instalação existente; registrar resultado e versão.
- Validar visualmente progresso, revisões e pendências, inclusive FEN inicial e
  posições repetidas, em classes de máquina representativas.
- Publicar amostras e divergências da comparação pareada e confrontá-las com as
  margens previamente fixadas. Se qualquer gate falhar ou faltar evidência,
  manter B experimental e preservar o caminho atual.

Nenhum destes gates é considerado cumprido apenas por este documento. A
validação visual integrada e a comparação completa continuam pendentes.
