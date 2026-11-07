# Comparação exploratória B

Este ensaio executa uma partida completa do corpus local: Larsen–Spassky,
Belgrado 1970 (a primeira de `games.pgn`). A seleção foi fixada antes da coleta;
ela é uma partida tática curta, não uma amostra representativa. O benchmark executa
em ordem A-B-B-A e termina com uma referência contextual mais profunda. Cada
execução começa com cache frio e processos novos, com um orçamento total de duas
Threads e 32 MiB de Hash. A é a pipeline automática Fast; B usa profundidades
8/12, MultiPV 3 e até dois workers; a referência usa a mesma política B e recursos,
mas profundidades 14/18. Não é verdade absoluta nem garantia de maior custo em
cada posição. O resultado inclui revisão inteira da pipeline A, snapshots completos
de B e referência, e tempos de parede incluindo criação e teardown dos processos.
Exclui IPC de frontend e renderização; a referência difere de A também quanto ao
histórico de posição, de modo que divergências não isolam o efeito da política.

Em `app/src-tauri/`, selecione um caminho de saída novo:

```sh
BENCH_REPORT_PATH=/tmp/b-paired.json cargo test --release --lib benchmark_b_paired -- --ignored --nocapture --test-threads=1
python ../benchmarks/analyze_b_paired.py /tmp/b-paired.json /tmp/b-paired-summary.json
```

O resumo reporta cada amostra separadamente e preserva denominador total, pendências,
comparabilidade e as divergências por lance. Empates entre melhores lances não são
considerados equivalentes. A referência pode ela própria conter pendências; faltas
não contam como acerto. Erro em centipawns somente é calculado quando ambos os
valores são comparáveis: mate B permanece distinto e valores grandes no formato
legado são excluídos conservadoramente. O resumo não compara classificação ou lance
jogado: o arquivo bruto é necessário para análise adicional. Duas amostras de uma
partida não estabelecem dispersão populacional, melhoria ou critério de adoção.
CPU e memória não são medidas por este experimento; não as infira do tempo.

A coleta em `docs/evidence/b-paired-2026-11-03` usou a política
`b-experimental-1`. O código atual usa `b-experimental-2`, que também verifica
conflitos inversos. Os comandos acima executam a política compilada atualmente;
use novos caminhos de saída e registre o commit, o estado do workspace, os hashes
do binário e corpus e a carga da máquina antes de comparar com a coleta anterior.
