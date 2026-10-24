# Chess.com Game Review — investigação com REA

Data: 13/10/2026. Alvo: `https://www.chess.com/analysis`, publicação web
`/r2/client-packages/analysis/2026.9.10/`.

## Conclusão principal

O Game Review combina **avaliação de engine, classificação contextual,
accuracy CAPS2 e explicações pedagógicas**. Usar Stockfish e calcular perda de
centipawns não reproduz, sozinho, a revisão do Chess.com.

O que foi estabelecido:

- A documentação descreve classificação por perda de **Expected Points**,
  dependente da avaliação e do rating, com cortes publicados.
- O navegador realmente executou **Stockfish 19 Lite**, usando WebAssembly e
  workers, na análise local do PGN de teste.
- Os scripts contêm fluxos de Game Review remoto via WebSocket, incluindo um
  protocolo JSON legado e uma migração binária/Protobuf condicionada por feature
  flag.
- Existe uma camada distinta, **CEE / Explanation Engine**, com interface WASM,
  avaliações, ratings, eventos de tabuleiro e recursos de texto/áudio de coaches.
- O frontend contém uma inversão logística dependente de rating para rotular o
  estado da posição. Isso é um achado concreto, mas não comprova a fórmula
  interna completa do classificador.
- A fórmula numérica completa de CAPS2, as regras exatas de classificações
  especiais e o comportamento autenticado do servidor **continuam abertos**.

### Tipos de evidência

- **Runtime observado:** ações e resultados capturados pelo REA no site real.
- **Código observado / interpretação estática:** bytes dos scripts entregues
  pelo site, com hashes; não significa que todos os caminhos executaram.
- **Documentação oficial:** descrição pública do produto; pode estar defasada
  em relação ao bundle atual.
- **Inferência:** reconstrução explicativa apoiada nessas fontes, identificada
  explicitamente quando ultrapassa o que foi observado.

## 1. Método e cobertura

Foi usada a skill `reverse-engineer-anything`, com os seguintes passos:

1. Captura REA em Chromium com perfil novo, sem login, da página de análise.
2. Retenção dos corpos de respostas e exportação de JavaScript pelo REA, sem
   refetch dos scripts durante a exportação.
3. Análise estática REA dos arquivos exportados.
4. Inspeção do módulo principal e leitura focalizada dos seus trechos originais.
5. Importação de um PGN artificial de mate do pastor, com ratings 1200/1200.
6. Clique no botão Game Review para verificar o fluxo acessível nessa sessão.
7. Confronto com artigos oficiais de classificação, accuracy, engines e review.

A análise REA registrou **196 arquivos JavaScript**, **10.146.670 bytes** de texto
e **1.763.929 nós AST visitados**, sem erros de parsing registrados. A cobertura
de aplicação foi **parcial** e a semântica foi **parcial/truncada**, com 367
unknowns de aplicação e 9.341 unknowns semânticos. O bundle grande e a análise
semântica atingiram limites de recursos; portanto, não há uma prova de cobertura
de todos os caminhos do frontend.

As capturas de navegador também têm cobertura incompleta de eventos: algumas
requisições ainda estavam em andamento e alguns corpos não ficaram disponíveis
quando a janela terminou. Isso não impede citar respostas e ações efetivamente
capturadas, mas impede concluir que nenhum outro evento existiria depois.

**Limite de acesso observado:** clicar em Game Review navegou inicialmente para
`/analysis/review`; depois a página voltou a `/analysis` e apresentou o diálogo
“Join Now”. Nenhum resultado completo de revisão autenticada foi obtido. Não
foram medidos accuracy ou rótulos finais do servidor para esse PGN. [E3]

## 2. Arquitetura recuperada

Visão reconstruída a partir do código e das descrições oficiais:

```text
PGN / partida + metadados de jogadores
                  │
         seleção de engine/perfil
                  │
          Game Review no servidor
                  │
        avaliações e linhas candidatas
                  │
      classificação / CAPS2 / CEE
                  │
  posições, resumo, temas, falas, key moves
                  │
    UI: gráfico, rótulos, coach, retry

Self Analysis / feedback de variações
                  │
        engine local WASM + workers
                  │
       avaliações → CEE local → UI
```

O diagrama resume responsabilidades; a ordem interna entre CAPS2, classificação
e CEE no backend não foi recuperada.

### Local versus servidor

Na execução observada, o site exibiu `Analysis depth=23 | Stockfish 19 Lite`.
Foram registrados workers e downloads de:

```text
/r2/assets-chess-engine/Stockfish/stockfish-19-lite-cfad2a3.js
/r2/assets-chess-engine/Stockfish/stockfish-19-lite-cfad2a3.wasm
```

Isso comprova o motor da **Self Analysis nessa execução**, não o motor do Game
Review remoto. [E1, E2]

O artigo oficial de engines, atualizado em agosto de 2026, descreve Game Review
server-side com Stockfish 18 e NNUE completo. A captura de outubro encontrou
Stockfish 19 Lite local. São contextos e datas distintos; não há confirmação da
versão efetivamente executada pelo servidor em outubro. O nome legado
`Stockfish16` em uma opção do frontend tampouco prova a versão real do backend.
[S4, E4, A2]

### Transporte remoto

O módulo principal contém os endpoints:

```text
wss://analysis.chess.com:443
wss://analysis.chess.com:443/v1/legacy/game-analysis
wss://analysis.chess.com:443/v2/game-review
wss://analysis.chess.com:443/v2/run-search
```

A flag `game_review_ceac_idl_migration` seleciona a implementação binária de
Game Review. O código contém uma proteção contra abrir o socket JSON legado no
endpoint binário. A presença desses caminhos é estática: a captura anônima não
validou uma troca autenticada nesses endpoints. [E4, E5, A1]

No caminho legado, a requisição é construída com:

```text
action: gameAnalysis
game.pgn
options.caps2: true
options.depth
options.engineType
options.strength
options.tep.classificationv3: true
options.tep.speechv3: true
```

No caminho binário, há metadados de rating, jogadores, posição inicial, variante
e controle de tempo. O mapeamento `bet` relaciona 18/22/24/26 com
Fast/Standard/Deep/Maximum. Isso é o valor codificado pelo cliente, não uma
medição de profundidade alcançada, tempo por posição ou política de refinamento
do servidor. [A2, A3]

## 3. Algoritmo de classificação

### Modelo documentado: perda de Expected Points

O Chess.com publica um modelo que depende de rating e avaliação da engine.
Seu resultado usa escala 0–1: 0 representa perda, 0,5 equilíbrio e 1 vitória.
Não se deve tratar essa descrição como uma decomposição WDL calibrada já
recuperada do servidor. [S1]

Reconstrução conceitual:

```text
E_best   = resultado esperado preservado pelo melhor lance
E_played = resultado esperado depois do lance jogado
loss     = perda entre essas duas avaliações, na perspectiva do jogador
```

Essa expressão explica o modelo; não é código de backend decompilado.

| Classificação | Limite inferior publicado | Limite superior publicado |
| --- | ---: | ---: |
| Best | 0,00 | 0,00 |
| Excellent | 0,00 | 0,02 |
| Good | 0,02 | 0,05 |
| Inaccuracy | 0,05 | 0,10 |
| Mistake | 0,10 | 0,20 |
| Blunder | 0,20 | 1,00 |

Os intervalos acima são os da documentação, intitulada **Classification V2**.
Ela não define desempate/inclusividade nos limites compartilhados nem todas as
exceções. O bundle atual solicita **ClassificationV3**; não foi demonstrado que
todos esses cortes e exceções permanecem idênticos nessa versão. [S1, A2, A5]

Exemplo ilustrativo, sem associar a um centipawn específico:

- De 0,95 para 0,92, a perda é 0,03: faixa documentada de Good.
- De 0,60 para 0,45, a perda é 0,15: faixa documentada de Mistake.

Isso explica por que uma queda grande em centipawns numa posição já
completamente ganha pode importar menos do que uma queda menor perto do
equilíbrio. O efeito depende da conversão avaliação → resultado esperado.

### Fórmula realmente encontrada no frontend

O helper `rKe`, usado por `aKe` para estados
`Winning / Better / Equal / Worse / Losing`, contém:

```js
Math.log(1 / r[e] - 1) / (.004 * ((.0685 * a - 12) / 100)) / 100
```

Nesse helper, `a` é selecionado comparando os ratings das brancas e das pretas;
para ratings numéricos válidos, equivale ao maior dos dois. Headers ausentes
têm default `GH = 2000`. Isso não estabelece o comportamento global para
rating zero, inválido ou ausente em outras camadas. [A4]

Escrita de forma legível, a inversão observada é:

```text
R = maior rating válido, no helper observado
k(R) = 0,004 × (0,0685 × R − 12) / 100
limiar_em_peões(q, R) = ln(1/q − 1) / k(R) / 100
```

As tabelas de `q` presentes no código são:

| Estado | Tabela usada para brancas | Tabela usada para pretas |
| --- | ---: | ---: |
| Winning | 0,20 | 0,80 |
| Better | 0,40 | 0,65 |
| Equal | 0,50 | 0,50 |
| Worse | 0,65 | 0,40 |
| Losing | 0,80 | 0,20 |

É evidência de uma transformação logística contextual no frontend. **Não é
prova de que essa inversão constitui a fórmula completa de Expected Points da
CEE, de CAPS2, ou de todos os rótulos de ClassificationV3.** A equivalência com
o classificador interno foi registrada como unknown `U2`.

### Brilliant, Great e Miss

Segundo a definição oficial: [S1]

- **Brilliant:** bom sacrifício de peça, com lance melhor ou quase melhor;
  a posição resultante não deve ser ruim e o jogador não deveria estar
  completamente ganho mesmo sem encontrar o lance. A concessão depende do rating.
- **Great:** lance decisivo, aproveitamento de uma mudança relevante de estado
  ou encontro do único bom lance. Há mais generosidade para jogadores novos.
- **Miss:** oportunidade de aproveitar erro do adversário e obter posição
  vencedora que não foi concretizada.
- **Book:** classificação específica de abertura, não simples faixa de perda.

O código observável usa eventos como `CriticalFind`, `FoundWin` e
`FoundNotLosing`, além de flags `isKeyMoment`, `book` e `isKeyMove`. Também
contém enumerações `Forced` e `Tricky`, sem que isso comprove sua exposição a
todos os usuários. [A5, A6]

Uma sutileza importante: “tornar uma posição igual em vencedora” pode descrever
o aproveitamento do erro adversário entre turnos. Não implica que um lance
melhor que o ótimo da mesma posição criou vantagem do nada.

Não foram recuperados os predicados completos para sacrifício, sua permanência,
compensação, thresholds de rating ou precedência entre todos os rótulos. [U3]

## 4. Accuracy: CAPS2

A documentação confirma **CAPS2**, sucessor de CAPS, e diz que a maioria dos
scores passa a se concentrar entre 50 e 95, com interpretação mais parecida
com uma nota escolar. Isso descreve uma escala de qualidade e não equivale
à porcentagem bruta de lances iguais à primeira escolha da engine. [S2]

O que o código permite afirmar:

- O caminho legado solicita `caps2: true`.
- `kqe` transporta `CAPS`, `reportCard`, `gamePhases`, `tallies`, `themes` e
  outros dados da resposta para o estado de revisão.
- `dZe` converte accuracy branca/preta recebida para `CAPS.white.all` e
  `CAPS.black.all`; não calcula a fórmula nesse adapter.
- Há resultados por fase e rating efetivo em adapters do relatório. [A2, A7]

**Não foi recuperada a fórmula exata de CAPS2.** Permanecem desconhecidos a
transformação por lance, os pesos de agregação, o tratamento de lances forçados,
livro, mates, posições decididas e partidas curtas. [U1]

Em particular, a fórmula exponencial e a combinação média ponderada/harmônica
usadas pelo EngineRoom não foram identificadas como CAPS2. Não é correto
apresentar o modelo do Lichess como a fórmula do Chess.com.

## 5. CEE, coach e feedback

O bundle contém metadados de `@chesscom/explanation-engine`, descrito como
“WASM build of internal Explanation Engine”, com versões 1.130.68 e 1.130.53.
Existirem duas versões no bundle não prova qual executou em cada feature. [A8]

No caminho local de feedback, o código:

1. Monta uma fila de avaliações por posição.
2. Solicita `multiPv: 2` e tempo de
   `window.chesscom.analysis?.moveFeedbackEngineTime ?? 3000` ms.
3. Ignora avaliações marcadas `upperbound/lowerbound` e seleciona linhas PV.
4. Ordena resultados por `moveId`.
5. Envia avaliações, lances, FEN, variante, `WhiteElo` e `BlackElo` à CEE.
6. Solicita `ScoreWhiteToMove`, `SerializeLikeCEAC`, `SerializeBoardEvents`,
   `ClassificationV3` e `SpeechV3`.
7. Usa eventos retornados para marcar posições críticas e atualizar a UI.

Esse é um caminho estático de **feedback local**, não a política de orçamento
do Game Review completo no servidor. [A5]

Há templates de fala associados a temas como peça presa, garfo, ganho de
material e mudanças de estado. Há ainda recursos de coach identificados por
`text_id`/locale e arquivos `cee_asset_v2.json.bzp`, além de seletores de emoção
e áudio. [A6, A9]

Isso sustenta uma camada explicativa estruturada sobre temas e variantes, com
recursos de linguagem e apresentação. Não demonstra que toda fala do produto
venha exclusivamente desses templates, nem permite afirmar a presença ou
ausência global de um LLM.

## 6. Por que avaliações e classificações mudam

A documentação distingue **Quick Analysis no dispositivo** de **Game Review
completo no servidor**. A revisão completa pode mudar um Brilliant ou Miss
inicial ao analisar mais profundamente. Perfis, motores, plataforma e força
também influenciam o resultado. [S3, S5]

Portanto, comparação de implementações precisa fixar pelo menos:

- versão do motor e rede NNUE;
- perfil/orçamento e profundidade efetivamente alcançada;
- MultiPV e avaliação do lance jogado;
- PGN, FEN inicial e ratings;
- versão da classificação e flags relevantes;
- perspectiva das avaliações e regras especiais de mate.

## 7. Relação com o EngineRoom atual

Leitura direta de `app/src-tauri/src/review/scoring.rs`:

- `ACCURACY_MODEL = "lichess-2026-10-initial-position"`.
- `win_pct` usa logística com coeficiente fixo `0.00368208`, limitando cp a
  `[-1000, 1000]`.
- `classify` usa cortes de perda em escala percentual 0/2/5/10/20 e Book.
- `move_accuracy` usa uma curva exponencial.
- `accuracy` combina média ponderada por volatilidade e média harmônica.

Os cortes percentuais lembram os cortes de Expected Points publicados, mas
**a conversão fixa sem rating e a accuracy do Lichess não comprovam equivalência
com o review do Chess.com**.

Implicações de projeto, derivadas desta investigação:

1. Manter modelos de accuracy explicitamente versionados e identificados.
2. Separar avaliação de engine, classificação, rótulos especiais e explicações.
3. Usar MultiPV e análise suficiente do lance jogado para unicidade/alternativas;
   considerar ruído entre pesquisas independentes.
4. Modelar sacrifício/compensação e oportunidades perdidas para Brilliant/Miss;
   uma queda em centipawns, isoladamente, não basta.
5. Produzir explicações verificáveis a partir de temas e linhas candidatas,
   apoiadas em eventos do tabuleiro.
6. Preservar a distinção entre análise preliminar e refinada, já relevante para
   os perfis adaptativos do EngineRoom.
7. Obter ratings do PGN quando um modelo contextual for implementado, conforme
   a arquitetura existente; a documentação não justifica novos campos no DB.

Essas são recomendações para uma implementação própria. Nenhuma mudança no
algoritmo do aplicativo foi realizada nesta investigação.

## 8. Checklist e questões em aberto

| Pergunta | Resultado |
| --- | --- |
| Há engine local? | Respondida por runtime: Stockfish 19 Lite WASM na execução observada. |
| O review completo é remoto? | Respondida por documentação e fluxos estáticos; execução autenticada pendente. |
| Qual é a regra geral de classificação? | Respondida pela documentação: perda de Expected Points contextual. |
| Há função dependente de rating no frontend? | Respondida: helper inverso `rKe`, com escopo limitado a estados da posição. |
| Qual é a fórmula exata de CAPS2? | Não resolvida. |
| Quais são todos os predicados de Brilliant/Great/Miss? | Parcial: definição pública e interfaces; núcleo interno não recuperado. |
| Como o coach é integrado? | Parcial: CEE, temas, templates e assets observáveis; runtime completo pendente. |
| O EngineRoom já é equivalente? | Não demonstrado; modelos e regras observáveis diferem. |

Próxima evidência necessária: uma sessão autenticada disponibilizada para
captura, com revisões de PGNs controlados e resultados completos. Casos úteis:
mate, defesa única, sacrifício bom/ruim, posição já ganha, book, forced e
oportunidade perdida; repetir alterando apenas os ratings e o perfil.

Unknowns formais registrados no REA:

- **U1 / CAPS2:** `unk_ed1dfdccf6d5def3d5938f28282ac9243961950671696b3d3e8a9e3d2a4bc227`.
- **U2 / equivalência do helper:** `unk_a557e01d41934973e5c83e503ad55cb7c513e14812d8e994c20f1a67d5b1757a`.
- **U3 / regras especiais:** `unk_370a744816d7e33911668160503fc820f347c47662adcdc78bebce804e54a860`.

## 9. Índice de evidências

Veja [`chesscom/evidence-index.json`](chesscom/evidence-index.json) para os IDs
integrais, hashes e âncoras. O bundle canônico REA foi preservado localmente em
`docs/research/chesscom/artifacts/evidence-bundle.json`, ignorado pelo Git por
conter corpos de respostas e análise extensa. O bundle pode incluir registros
anteriores da mesma sessão; esta investigação usa os IDs identificados abaixo.

Também foram preservados `artifacts/main-asset.js` e
`artifacts/source-anchors.json`, com os trechos originais A1–A9. As coordenadas
do índice são offsets UTF-16, começando em zero, com início inclusivo e fim
exclusivo. Foram verificados o hash do asset, a presença dos cinco registros
E1–E5 e dos três unknowns no bundle, as nove âncoras e a correspondência exata
do PGN salvo com o input capturado. O hash do bundle está no índice.

| Alias | Observação |
| --- | --- |
| E1 | Captura inicial do site e respostas JavaScript. |
| E2 | Importação do PGN e execução da Self Analysis local. |
| E3 | Clique Game Review e diálogo de cadastro. |
| E4 | Análise estática REA do conjunto de scripts exportados. |
| E5 | View REA do asset principal, identidade e observações estáticas. |
| A1–A9 | Âncoras de código original no asset principal, documentadas no índice. |

Fontes oficiais consultadas em 13/10/2026:

- **S1:** [How are moves classified?](https://support.chess.com/en/articles/8572705-how-are-moves-classified-what-is-a-blunder-or-brilliant-etc)
- **S2:** [How is accuracy in Analysis determined?](https://support.chess.com/en/articles/8708970-how-is-accuracy-in-analysis-determined)
- **S3:** [How does Game Review work?](https://support.chess.com/en/articles/8584089-how-does-game-review-work)
- **S4:** [How do the chess engines on Chess.com work?](https://support.chess.com/en/articles/9462780-how-do-the-chess-engines-on-chess-com-work)
- **S5:** [Why did my move classification change?](https://support.chess.com/en/articles/11845102-why-did-my-move-classification-change-in-game-review)
- **S6:** [How is Game Rating calculated?](https://support.chess.com/en/articles/10773754-how-is-game-rating-calculated-in-game-review)

Game Rating é uma estimativa de desempenho na partida, distinta da accuracy
e do rating competitivo; sua fórmula exata também não foi recuperada. [S6]
