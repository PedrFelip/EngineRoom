# Ensaio exploratório pareado — 2026-11-03

A coleta usou o protocolo em `app/benchmarks/B-PAIRED.md`, com a primeira
partida completa do corpus (34 plies) e cache frio. A ordem foi A-B-B-A e,
por último, uma referência contextual mais profunda. Os dados integrais estão
em `raw.json.gz` e as comparações por lance em `summary.json`. São resultados
de uma partida numa máquina, não uma avaliação suficiente para adoção.

| Amostra | Parede, com teardown | Melhor lance igual à referência | Erro absoluto médio CP |
|---|---:|---:|---:|
| A Fast 1 | 16.286 ms | 18/29 comparáveis | 42,1 (29 comparáveis) |
| B 1 | 1.751 ms | 19/29 comparáveis | 76,5 (29 comparáveis) |
| B 2 | 1.788 ms | 19/29 comparáveis | 76,5 (29 comparáveis) |
| A Fast 2 | 16.794 ms | 16/29 comparáveis | 35,8 (29 comparáveis) |
| Referência B 14/18 | 18.975 ms | — | — |

Há 34 decisões no denominador completo. B e referência reportaram pendência
nos plies 19, 31, 32, 33 e 34, todos por persistência de resultado crítico após
o orçamento de verificação; eles não entram nas 29 comparações numéricas.
Essa exclusão remove precisamente casos críticos, inclusive mates, e impede
interpretar as médias como qualidade geral. A referência também não é verdade.
Empates entre melhores lances contam como divergência. Resultados idênticos das
duas amostras B não demonstram estabilidade em outros jogos ou máquinas.

Inspeção dos dados brutos: no ply 19 (`f4`) B e referência registram resultado
crítico, com melhor `a2a3`, mas permanecem pendentes. No ply 24 (`...h4`), B
sugere `d8d2` com +246 cp em profundidade 8; a referência sugere `h5h4` com
+571 cp em profundidade 18. No ply 30 (`...g2`), ambos sugerem `g3g2`, mas B
registra melhor +91 cp e jogado +677 cp, enquanto a referência registra +578 e
+790 cp. Essas avaliações misturam buscas independentes e profundidades e merecem
investigação; não provam qual conclusão é correta. Os plies 31–34 incluem mates
que foram preservados como mate, sem convertê-los em diferença de centipawns.

### Origem da divergência no ply 30

O screening usa a evidência 30 para a posição anterior: profundidade 8, +91 cp,
PV `g3g2 h1g1 e7h4 e1d1`. A evidência 31, na posição posterior, tem -100 cp
na perspectiva do jogador seguinte e é invertida para +100 cp. Ao verificar o
ply 31, B produz evidência 42 para a mesma posição posterior, profundidade 12 e
-677 cp, invertidos para +677 cp. A revisão do ply 30 combina essa evidência
com a evidência 30 ainda em profundidade 8; as duas citam sua origem corretamente.
O sinal não está invertido incorretamente: as buscas independentes e a propagação
de uma evidência mais profunda explicam a assimetria observada, sem provar a
verdadeira avaliação da posição. A referência exibe o mesmo padrão com evidência
de screening 30 em profundidade 14 (+578 cp) e verificação 40 em profundidade 18
(-790 cp no filho, invertido para +790 cp).

O predicado atual abre investigação apenas se `melhor - jogado >= 100 cp` ou
houver mate. Portanto não sinaliza uma discrepância invertida, mesmo quando o
jogado supera o suposto melhor por 586 cp; a conclusão continua não crítica.
Isso é consistente com a implementação unilateral, mas deixa uma lacuna frente
ao objetivo arquitetural de explicitar resultados conflitantes. Antes da adoção,
é necessário definir uma política de verificação para conflito nessa direção,
com orçamento finito e pendência quando persistir, e testá-la com este caso.

**Mudança posterior à coleta:** `b-experimental-2` passou a verificar também
conflitos invertidos e a distingui-los de perdas críticas. Os dados desta coleta
descrevem a política anterior, `b-experimental-1`; não medem custo, pendências
ou qualidade da política nova. Repita a experiência após congelar a versão.

Ambiente: Ryzen 5 3600X, 12 CPUs lógicas, 31 GiB de RAM, Linux
`7.2.15-1-omarchy` x86_64, Rust `1.98.1`. O executável Stockfish tem SHA-256
`0f83d24cc46d2c66c60f16001af5444873bc112b7d028594513426894c12da19`;
o corpus `games.pgn`, `d797f90590fd0043fe221228e8e684a73a8611524dcb2cc569ddaab49e281950`;
o binário de teste release usado, `bc4fdc16f03fb826c533cbb8b7ac79175049a7c183f2384258dcef03b760f715`.
O HEAD era `134ed5fa0a8a09cdea7cf6e8f8a7434d3cdc58c8` **com alterações locais
não commitadas e trabalho paralelo**, portanto o HEAD não identifica o código
completo executado. Pode haver carga concorrente; os tempos não constituem uma
baseline controlada. CPU, pico de memória e IPC/UI não foram medidos. A usa
SQLite em memória e B usa runner sem cache; contexto, seleção e concorrência
variam simultaneamente. A versão consolidada deve ser congelada e a coleta
repetida em corpus maior antes de qualquer decisão.
