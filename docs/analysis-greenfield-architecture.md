# Alternativa B: revisão orientada a decisões e evidências

Status: proposta greenfield, sem compromisso com os módulos, protocolo ou banco atuais. Não implementada nem validada por benchmarks. Elaborada com três agentes de arquitetura, correção e avaliação, com confronto das alternativas.

Esta proposta responde aos [requisitos da análise](analysis-pipeline-requirements.md), mas permite rever suas decisões quando isso melhorar o produto. A [proposta incremental](analysis-pipeline-design.md) preserva a pipeline atual; aqui a pergunta é qual sistema construiríamos se começássemos hoje.

O [plano de implementação de B](analysis-b-implementation-plan.md) divide esta proposta em tarefas, dependências e critérios de conclusão.

## Escolha principal

Construir um **sistema de revisão progressiva orientado a decisões**, no qual cada lance gera perguntas de análise, resultados do motor são evidências e a classificação é uma conclusão revisável.

A unidade central deixa de ser “analisar esta posição por X milissegundos”. Passa a ser “o que precisamos verificar para julgar este lance?”. Uma posição pode ajudar vários lances; um lance pode exigir várias buscas. Tempo e profundidade são meios de adquirir evidência, não a definição de uma decisão resolvida.

Não começar por um pool, um framework de DAG ou uma estratégia de continuidade. Começar pelos contratos de pergunta, evidência, conclusão e orçamento. O scheduler decide qual investigação executar; o Stockfish continua sendo o instrumento de busca local.

## O que muda de verdade

| Dimensão | Pipeline de duas etapas | Proposta greenfield |
| --- | --- | --- |
| Unidade de produto | Posição avaliada | Decisão sobre um lance |
| Organização | Triar posições, selecionar, refinar | Cobrir decisões e cumprir obrigações de evidência |
| Resultado | Avaliação final escolhida | Conclusão versionada com evidências identificadas |
| Incerteza | Instabilidade usada para refinamento | Pendências explícitas: alternativa ausente, resultado conflitante, mate não confirmado |
| Cache | Resultado compatível por orçamento | Evidência contextual com assinatura de motor e escopo |
| Scheduler | Filas de triagem e refinamento | Ações elegíveis que contribuem para decisões |
| Término | Todas as etapas programadas concluídas | Contrato de cobertura cumprido ou orçamento encerrado com pendências explícitas |
| Continuidade | Hipótese de organização da pipeline | Otimização oportunista de uma execução |

Isso não elimina buscas gerais por posição nem uma passagem inicial barata. Elimina a necessidade de representar todo o produto como exatamente duas etapas.

## Modelo de domínio

```text
Review
  game_context
  analysis_contract
  decisions[ply]
  evidence_store
  resource_ledger

DecisionCase
  id
  before_context
  played_move
  after_context
  screening_snapshot?
  obligations[]
  conclusion_revisions[]

EvidenceRequest
  context_id
  question_kind
  search_scope
  requested_coverage
  engine_signature

Evidence
  id
  request_id
  execution_id | cache_origin
  scores_and_lines
  observed_coverage
  completion_reason
  accounting

ConclusionRevision
  decision_id
  revision
  supporting_evidence_ids
  classifier_version
  classification
  limitations
  pending_obligations
```

Scores devem distinguir centipawns e distância de mate, com perspectiva explícita. Bounds do protocolo são tipados; `Exact` significa ausência de bound reportado, não verdade matemática da avaliação. WDL, se utilizado, precisa de semântica e configuração registradas. Um conjunto MultiPV aceito exige linhas coerentes, raízes legais e distintas; preservar o último conjunto completo quando uma atualização chega parcial.

Esses são tipos específicos do xadrez. Não são um sistema genérico de agentes autônomos ou um workflow configurável pelo usuário.

### Perguntas e obrigações

O conjunto inicial deve ser pequeno:

- `BestMove`: qual alternativa a busca recomenda na posição anterior?
- `PlayedMoveValue`: qual é a avaliação da continuação escolhida?
- `AlternativeCoverage`: as alternativas necessárias para o julgamento foram observadas?
- `VerifyCritical`: existe evidência adicional suficiente para sustentar um diagnóstico de grande impacto?
- `ResolveConflict`: duas evidências compatíveis em contexto discordam de maneira relevante?

Uma obrigação não é prova matemática. É um critério operacional explícito para uma revisão útil. O sistema deve poder dizer “análise insuficiente” em vez de apresentar certeza não demonstrada.

As primeiras obrigações podem ser satisfeitas por buscas na posição anterior e na posição seguinte. O resultado desta última também pode servir à próxima decisão, desde que o contexto e o escopo coincidam. É aí que o grafo aparece: evidências compartilhadas, sem obrigar buscas duplicadas.

### Contexto é parte da identidade

`PositionContext` contém posição inicial, sequência legal de lances até o ponto analisado e limite do histórico conhecido. PGN iniciado em FEN não revela o que aconteceu antes dela.

A chave lógica da evidência inclui contexto, motor/build, opções semanticamente relevantes, escopo e política de regras. FEN é um campo útil para consulta, não identidade suficiente por definição. No primeiro desenho, guardar o contexto conhecido completo é preferível a uma canonicalização histórica difícil de provar. Usar digest com payload verificável; não confiar apenas no digest para validar equivalência.

A política para empate reivindicável, repetição e cinquenta lances é explícita e versionada. Ela afeta conclusões e reaproveitamento. Mesma posição de peças não implica mesma pergunta.

### Comparar o lance jogado com a alternativa

Não assumir que subtrair dois scores produz uma perda exata. Buscas independentes têm horizontes e cobertura diferentes; scores são estimativas.

Para o MVP, usar busca irrestrita na raiz e avaliação do filho com perspectiva normalizada, registrando a assimetria. Quando isso sugerir decisão crítica, agendar verificação adicional. Um experimento posterior pode comparar o lance jogado e alternativas na mesma raiz mediante busca restrita; isso muda o escopo da evidência e não substitui automaticamente a busca irrestrita de melhor lance.

MultiPV fornece cobertura de alternativas, mas não garante que o lance jogado esteja entre elas. O contrato precisa identificar quais raízes foram efetivamente avaliadas, em vez de inferir isso da quantidade de linhas.

## Componentes de execução

Fluxo: `UI → SessionActor → DecisionPlanner → ExecutionBroker → EngineActors`.

`EvidenceStore` e `ResourceLedger` atendem o ator de sessão; persistência e telemetria ficam fora da lógica de decisão.

| Componente | Responsabilidade |
| --- | --- |
| `SessionActor` | Único escritor do estado da revisão; serializa navegação, evidências, cancelamento e publicação |
| `DecisionPlanner` | Funções puras para abrir obrigações, produzir conclusões e ordenar ações elegíveis |
| `ExecutionBroker` | Capacidade global local, leases, orçamento reservado, despacho e preempção |
| `EngineActor` | Um processo, um leitor UCI, uma execução ativa, configuração e término reconhecido |
| `EvidenceStore` | Resultados imutáveis, índice de compatibilidade, leitura/gravação em lote |
| `ResourceLedger` | Autorizações, reservas e consumo nominal/efetivo por revisão e execução |
| `ReviewProjection` | Estado compacto para UI: progresso, classificação, limitações e revisão |

“Actor” significa uma tarefa com mailbox tipada e estado próprio; não exige biblioteca ou infraestrutura distribuída. O planejador não roda lógica bloqueante no ator; parsing pesado e banco executam fora dele e retornam eventos identificados.

O broker pode ser global ao aplicativo e admitir inicialmente um único owner de análise, preservando simplicidade. Várias sessões simultâneas só justificam compartilhamento e fairness entre sessões se o produto precisar disso.

## Política inicial do scheduler

Não começar com um estimador numérico de “valor da informação”. Sem calibração, multiplicar impacto, confiança e custo pode dar aparência científica a pesos arbitrários.

Usar prioridades lexicográficas determinísticas:

1. Cancelamento, prazo e limpeza de recursos.
2. Navegação interativa mais recente, dentro da capacidade autorizada.
3. Piso de cobertura das decisões ainda sem evidência mínima.
4. Verificações críticas obrigatórias já admitidas.
5. Conflitos e alternativas ausentes que bloqueiam conclusões úteis.
6. Aprofundamento opcional.

Dentro de cada classe, usar idade, dependências desbloqueadas, custo estimado e afinidade como desempates documentados. Garantir progresso por reservas de capacidade e aging; uma lista de prioridade sem regra de admissão pode causar starvation.

O piso de cobertura recebe capacidade garantida. No modo revisão automática, hard checks recebem uma faixa limitada durante essa cobertura; com um worker, alternância limitada evita bloquear a partida inteira atrás de uma investigação. Tráfego interativo contínuo pode suspender o avanço automático; a UI deve mostrar essa suspensão, sem prometer um prazo que não será cumprido.

As ações concretas são poucas: consultar evidência, iniciar busca, continuar execução autorizada, parar execução e publicar revisão. A decisão sobre processos vem depois da capacidade de CPU/memória, não do número de decisões no grafo.

## Revisão progressiva sem esconder mudanças

Há dois objetos distintos:

- **Screening snapshot:** observação inicial imutável, usada para auditoria e comparação da seleção inicial.
- **Conclusão:** interpretação versionada do conjunto de evidências disponível.

Uma nova busca pode corrigir uma classificação e abrir nova obrigação. Ela não reescreve o screening antigo: registra nova revisão e motivo. Assim, a seleção inicial permanece auditável sem transformar seu possível erro em verdade permanente.

Esse é um delta deliberado em relação à leitura estrita de uma baseline que governa toda a seleção final. A proposta greenfield permite descoberta posterior; exige registrar sua origem e custo. Benchmarks de seleção congelada continuam possíveis usando o snapshot e um manifesto de obrigações fixas.

Estabilidade de melhor lance, profundidade ou proximidade de scores pode orientar investigação. Não é “95% de confiança” nem probabilidade de acerto sem calibração independente.

## Contratos de orçamento e término

Escolher um contrato por revisão; não prometer ao mesmo tempo prazo rígido e análise ilimitada de todas as posições críticas.

| Contrato | Promessa | Término |
| --- | --- | --- |
| Cobertura | Cumprir piso inicial e executar protocolos obrigatórios de verificação | Protocolos executados ou encerrados pela política; perguntas não resolvidas explícitas; duração variável |
| Prazo | Parar novas admissões antes do limite e iniciar limpeza | Resultado parcial com pendências; overshoot de término medido |
| Exploração | Continuar melhorando enquanto autorizado | Usuário encerra; publica estado acumulado |

Um prazo precisa declarar o relógio: parede da revisão ou orçamento de CPU. Para o MVP, orçamento nominal de busca e limite de parede são mais simples de controlar; CPU fica medição até existir mecanismo validado de controle.

Antes de iniciar uma ação, o broker reserva seu custo máximo nominal. Concluir libera saldo e registra consumo. Todo custo especulativo e espera com busca ativa pertence à revisão. Encerrar por deadline não se apresenta como “mesma qualidade, apenas mais rápido”.

No contrato de cobertura, limitar expansão recursiva de investigações: regras finitas, limite de tentativas e saída explícita `unresolved`. Caso contrário, “resolver conflito” pode gerar trabalho sem fim. Cumprir uma obrigação significa cumprir o protocolo de verificação definido; não provar que o score está correto.

O recurso inclui processos residentes, buscas ativas, Threads, Hash e memória adicional. Um processo ocioso não equivale a uma busca pausada. A capacidade deve reservar margem para o app e o sistema; valores finais dependem de medição.

## Evidência, cache e persistência do zero

Usar armazenamento contextual versionado desde o início, com três categorias lógicas:

1. Partida e revisão: metadados, contrato e conclusões.
2. Evidências de engine: contexto, assinatura, escopo e resultados completos reutilizáveis.
3. Trace experimental: execução, custos e eventos diagnósticos, com retenção limitada.

Não persistir cada linha de stdout. Persistir evidências aceitas e revisões relevantes; coalescer progresso. Um SQLite local pode atender as três categorias com tabelas e políticas distintas. Não há motivo demonstrado para microserviços ou banco de grafos.

A compatibilidade é predicado explícito. MultiPV, profundidade, orçamento de tempo, roots avaliadas e modo de término não se fundem por máximos independentes. Uma execução interrompida não herda a duração originalmente autorizada. Evidência experimental ou de busca restrita não contamina o cache geral.

Reaproveitar um resultado poupa trabalho, mas não restaura memória de engine. O trace registra essa diferença. Histórico salvo referencia evidências ou incorpora um snapshot suficiente para sobreviver à política de descarte do cache; excluir cache não pode destruir uma revisão salva.

## Ciclo de vida e falhas

Todo evento inclui sessão, geração, pergunta e execução. O ator rejeita eventos antigos antes de alterar conclusões. A revisão publicada tem número próprio, permitindo à UI descartar atualizações obsoletas sem entender UCI.

O broker só devolve capacidade depois de término reconhecido. Cancelar um future não encerra um processo. Depois de interrupção ou erro que torne o estado duvidoso, destruir e aguardar o engine actor antes de reutilizar a capacidade. Fechamento reconhecido inclui processos e gravações possuídas pela sessão.

Separar disponibilidade da engine de durabilidade do resultado. Se a persistência falhar, o produto precisa decidir explicitamente entre falhar a revisão ou continuar em modo temporário informado; não declarar salvamento bem-sucedido. O primeiro MVP pode manter a política conservadora de falha fatal.

Progresso descartável tem fila limitada e coalescida. Evidência aceita, falha e encerramento usam entrega confiável. A leitura do motor não pode depender da velocidade de renderização.

## Papel da continuidade

Continuidade é uma otimização do broker: uma execução já ativa pode satisfazer outra obrigação compatível, desde que haja orçamento, configuração e prioridade adequados.

Ela não é garantida. Se faltarem dependências ou surgir navegação, parar e reiniciar pode ser a melhor escolha. Registrar mesma busca, novo `go` no mesmo processo e processo novo separadamente.

Para o primeiro produto greenfield, usar Stockfish sem modificações, buscas separadas e MultiPV específico da pergunta. Investigar continuidade somente depois de medir o modelo de evidências. Um fork com pausa ou migração de estado acrescentaria uma dependência de manutenção antes de provar benefício.

## MVP que eu construiria

1. `PositionContext`, `DecisionCase`, perguntas fechadas e evidência imutável.
2. Driver de engine com conclusão coerente; inicialmente sem streaming de checkpoints públicos.
3. Ator de sessão e um broker simples, com um worker como configuração mínima e concorrência limitada parametrizada.
4. Cobertura inicial compartilhando avaliações entre decisões vizinhas.
5. Verificação das decisões críticas e conclusões versionadas.
6. Cache contextual e histórico independente da retenção do cache.
7. Contrato de cobertura como modo inicial; prazo como modo distinto, depois de existir UI para pendências.
8. Somente então: otimização de afinidade, janela, continuidade e sizing adaptativo.

O grafo pode ser representado por arrays de decisões e índices de dependências. Um framework de DAG só seria justificável se essa implementação concreta ficasse insuficiente.

## Como decidir se vale substituir a arquitetura atual

A arquitetura nova pode ser melhor conceitualmente e pior como investimento. Uma reescrita só se justifica se o modelo atual impedir funcionalidades ou ganhos comprovados.

Construir uma fatia vertical isolada que percorra PGN → perguntas → engine → evidência → conclusão para uma partida inteira. Comparar com o produto atual, sem migrar UI ou banco no primeiro ensaio.

Avaliar separadamente:

- Correção: contexto histórico, ausência de evidência falsa, regras de empate e classificação auditável.
- Utilidade: erros graves encontrados, alternativas úteis e divergências investigadas contra referência mais cara.
- Custo: parede, CPU, memória, latência de navegação e cancelamento, e trabalho descartado.
- Complexidade: quantidade de regras de scheduler, estados de recuperação e custo de adicionar uma nova pergunta de análise.

Usar replay determinístico, Stockfish com obrigações congeladas e revisão real completa. Igualar recursos e distinguir orçamento de cobertura; escolher métricas e margens antes dos resultados. Não comparar o contrato de prazo parcial com a revisão completa como se fossem a mesma entrega.

A recomendação de desenho do zero é este modelo por decisões e evidências. A recomendação de execução hoje é validar sua fatia vertical antes de autorizar uma reescrita. Se a experiência e os custos não melhorarem, incorporar apenas contratos úteis à base existente continua sendo um resultado válido.
