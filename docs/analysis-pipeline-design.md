# Alternativa A: planejador por posições e etapas

Status: proposta para discussão; não altera a implementação nem aprova continuidade em produção.

Este desenho traduz os [requisitos](analysis-pipeline-requirements.md) em responsabilidades e decisões implementáveis. Complementa o [experimento de continuidade](local-analysis-architecture.md) e preserva a [afinidade oportunista](triage-refinement-affinity.md). Foi elaborado com análises independentes de agendamento, correção e avaliação, seguidas de confronto das propostas.

Uma alternativa sem compromisso com a base atual está em [arquitetura orientada a decisões e evidências](analysis-greenfield-architecture.md). Ela muda a unidade principal do produto e os contratos de conclusão; este documento trata da evolução incremental.

## Recomendação

Organizar a análise em **planejador puro, executor de buscas e serviços existentes de sessão e persistência**. O trabalho lógico pertence ao planejador; processos e comandos UCI pertencem ao executor. A política inicial continua sendo a fila mista atual. Continuidade com janela limitada será uma segunda política experimental, condicionada a evidência de qualidade e custo.

O ganho arquitetural pretendido é conseguir responder, para cada posição: o que falta entregar, por que ela foi selecionada, quais execuções ocorreram, o que foi efetivamente produzido e quanto custou. Preservar uma busca é uma possível otimização dessas decisões.

Não introduzir framework genérico de workflow, armazenamento de árvore de busca ou novo serviço externo. Começar com tipos e funções internos em Rust e extrações pequenas do código atual.

## Alternativas consideradas

| Alternativa | Vantagem | Limite | Decisão proposta |
| --- | --- | --- | --- |
| Evoluir apenas a fila atual | Menor mudança; preserva comportamento conhecido | Estado de seleção, cobertura e execução permanece acoplado | Usar como ponto de partida e controle experimental |
| Planejador puro + executor; buscas separadas | Torna decisões reproduzíveis; mantém MultiPV específico de cada etapa | Exige explicitar estados hoje espalhados | Arquitetura recomendada |
| Janela deslizante com promoção no mesmo `go` | Pode preservar busca ativa para candidatos descobertos cedo | Ocupa recursos esperando vizinhos; mantém MultiPV de triagem | Política experimental |
| Processo residente por posição | Preserva afinidade sem redistribuição | Memória e quantidade de processos crescem com a partida | Rejeitar como modelo geral |
| Fork do Stockfish com pausa/retomada | Poderia oferecer outro contrato de continuidade | Viabilidade e custo de manutenção não demonstrados | Adiar; não é dependência |

Os ensaios existentes demonstram transição contínua em casos pequenos. Não demonstram vantagem sobre a fila atual nem validam revisão de partida inteira.

## Responsabilidades e fronteiras

Fluxo: `ReviewSession → AnalysisRunner → Planner + SearchDriver + Repository`.

| Componente | Possui | Não decide |
| --- | --- | --- |
| `ReviewSession` existente | Vida da sessão, intenções, request IDs, fechamento e lease | Ranking de lances |
| `Planner` puro | Trabalhos, baselines, seleção, cobertura exigida, prioridade e reconciliação | Spawn, relógio real, SQL ou comandos UCI |
| `AnalysisRunner` | Loop assíncrono, despacho de ações, resultados do cache, relógio e publicação | Regras de classificação duplicadas |
| `SearchDriver` sobre `EnginePort` | Uma execução, parsing, checkpoints, limites e término | Quotas ou seleção global |
| `Pool`/owner existente | Processos e streams exclusivos, configuração e teardown | Verdade sobre satisfação de um alvo |
| `Repository` existente | Consultas em lote, gravações e histórico | Equivalência inventada entre resultados experimentais e cache legado |

`adaptive.rs` permanece a autoridade das regras de seleção. O planejador o chama com baselines congeladas. `EngineFactory`, `EnginePort` e `Repository` continuam seams de teste; não substituir o transporte por um contrato acoplado ao Tauri.

O primeiro planejador pode ser um estado concreto com funções `accept(event)` e `next_actions(resources)`. Só introduzir uma interface de políticas quando existir a segunda implementação. Não duplicar a seleção Rust no harness Python.

## Modelo de domínio mínimo

```text
PositionContext {
  initial_fen, moves_uci_until_position, current_fen,
  history_completeness
}
WorkId = (review_generation, position_index)
SearchWork {
  id, position_context,
  baseline?, requirements, selection_reasons,
  executions, accepted_result?, status
}
CoverageRequirement {
  mode, requested_value, required_multipv, search_scope
}
SearchExecution {
  id, work_id, worker_id, engine_identity, options,
  authorized_limit, timestamps, outcome, reuse_kind
}
Checkpoint {
  execution_id, sequence, received_at,
  coherent_depth, exact_lines, engine_counters
}
```

São contratos conceituais, não um compromisso de serialização ou nomes públicos. `search_scope` distingue busca irrestrita de eventual busca restrita; não introduz `searchmoves` no produto. Um resultado restrito não satisfaz silenciosamente uma solicitação irrestrita.

Separar três perguntas:

1. **Cobertura:** a execução atende modo, valor, MultiPV e contexto exigidos?
2. **Aceitação:** suas linhas e avaliações podem substituir o resultado exibido?
3. **Persistência:** o resultado pode ser representado com segurança no cache atual?

Um refinamento pode consumir seu orçamento e satisfazer a execução solicitada sem substituir uma baseline mais profunda. Registrar ambos; não refazer indefinidamente só porque o resultado não foi escolhido para exibição. Inicialmente preservar a regra atual de exibição que rejeita refinamento menos profundo; mudar essa regra exige experimento próprio.

Manter certificados de cobertura por resultado, sem combinar máximos independentes: uma busca de 2.000 ms/MultiPV 1 e outra de 500 ms/MultiPV 2 não comprovam 2.000 ms/MultiPV 2. Cada pedido deve ser satisfeito por um resultado compatível, sem soma implícita de execuções.

`reuse_kind` distingue cache, mesma busca ativa, novo `go` no mesmo processo e processo novo. Checkpoint é observação imutável, nunca estado retomável.

## Correção de posição e cache

O `Game` atual já contém FEN inicial e lances UCI. Construir `PositionContext` a partir desses dados e fornecer ao driver a posição inicial com o prefixo de lances. Para PGN iniciado por FEN, o contexto anterior àquela FEN é desconhecido; registrar esse limite, sem inventar ocorrências anteriores.

A chamada atual `engine::evaluate` envia somente `position fen`. Isso conserva os campos da FEN, mas não a sequência necessária para repetição. Identidade de trabalho por índice evita deduplicação acidental entre ocorrências; compartilhar resultados entre trabalhos exige equivalência contextual explícita.

O cache atual usa `(fen, reached_depth, multipv)` e não representa esse histórico. A primeira execução experimental com contexto deve desviar de leitura e escrita do cache legado quando sua equivalência não estiver demonstrada. Não anexar um hash de histórico à string FEN nem alterar a consulta de cobertura disfarçadamente. Para adoção completa, decidir entre provar uma classe segura de reutilização e criar um namespace contextual com migração explícita. Um bypass amplo reduz hits; medir esse custo separadamente do scheduler.

Manter os contratos atuais: profundidade aceita cobertura alcançada suficiente; tempo só aceita entrada por tempo com orçamento suficiente; MultiPV suficiente em ambos. Preservar consultas em lote, flush a cada oito conclusões e ao final, JSON corrompido como miss, I/O real fatal e flush de falha sem substituir o erro original. Checkpoints experimentais não entram no cache legado.

## Baseline e checkpoints

O driver precisa de um assembler de conjuntos completos de linhas. O parser atual já descarta bounds, mas `evaluate` mantém a última linha de cada MultiPV; essas linhas podem ter profundidades diferentes. `RawPosition` continua DTO compatível com o produto, não vira automaticamente um checkpoint coerente.

Para o caminho experimental:

- Aceitar somente linhas exatas, PVs legais com primeiros lances distintos e slots `1..min(MultiPV, lances legais)` completos na mesma profundidade; manter o último conjunto coerente.
- Rejeitar conjuntos ambíguos; UCI não fornece um identificador universal de iteração. Não prometer atomicidade que o stream não demonstra.
- Congelar a baseline uma vez, com corte temporal definido pelo timestamp de recepção do driver. Linhas posteriores podem melhorar o resultado, mas não reescrever a justificativa da seleção.
- Se não houver conjunto coerente no prazo, marcar baseline incompleta. Não inventar zero nem tratar ausência como estabilidade; aplicar recuperação limitada e contabilizada ou retornar falha explícita.
- Tratar posições terminais antes de agendar motor. A política de empate reivindicável e de repetição deve ter testes próprios e não ser confundida com mate/afogamento.

A recuperação para baseline incompleta precisa de parâmetro experimental explícito. Não aumentar silenciosamente os orçamentos dos perfis.

## Agendamento inicial: equivalência com a fila atual

Extrair primeiro o estado de `automatic.rs` preservando:

- Blocos roubáveis de quatro posições de triagem.
- Faixa limitada de refinamento hard antecipado; progresso da triagem com um worker.
- Seleção global final com as mesmas regras e quotas soft.
- Deduplicação de requisitos por posição, reconsulta de cache quando a exigência aumenta e nenhuma execução concorrente duplicada para o mesmo trabalho.
- Afinidade por recência, depois origem, depois worker livre; nenhum worker fica esperando afinidade.
- Publicação e persistência por conclusão.

A baseline continua imutável. Concluir a triagem de todas as posições dispara reconciliação global; conclusão da revisão exige que todos os requisitos finais estejam satisfeitos. Alvos antecipados que saiam da seleção permanecem contabilizados como custo gasto, sem substituir obrigações finais.

Não é necessário reabrir um estado terminal para refinamento: `baseline_ready`, `requirements_satisfied` e `execution_state` são dimensões separadas. A revisão só termina após a reconciliação; novos requisitos podem aparecer antes disso.

## Política experimental de execução

A proposta de [janela deslizante com continuidade](analysis-window-pipeline.md) está em documento próprio. É uma política alternativa ao despacho atual, com promoção local, espera explicitamente orçada e reconciliação global; não muda por si só o modelo de domínio deste planejador.

## Orçamento e recursos

`profiles.json` continua fonte única. No contrato atual, triagem e refinamento são buscas com seus respectivos orçamentos; não reinterpretar o refinamento como saldo da triagem. Para comparar continuidade, declarar o total equivalente autorizado e como execuções anteriores, espera e overshoot entram na conta.

Introduzir primeiro um ledger de observação: orçamento nominal autorizado, busca efetiva, CPU agregada, parede, espera, setup, teardown e overshoot. CPU é medição; não alegar teto rígido a partir de `movetime`. Não somar dois `go` e apresentar o total como pesquisa contínua equivalente.

Um teto global rígido por partida não existe como decisão aprovada aqui. Ele pode conflitar com a obrigação de concluir todos os hard targets. Se for introduzido, o produto deve definir qual garantia cede e publicar estado de cobertura incompleta. Até lá, limitar especulação experimental sem descartar alvos finais obrigatórios.

Manter inicialmente o sizing atual como controle. Uma política posterior pode considerar CPU efetivamente disponível, memória disponível e margem para o sistema; Hash não representa RSS total. Reconfigurar somente workers ociosos, respeitando a exclusividade. Medir recursos e responsividade antes de alterar a distribuição de Threads/Hash.

## Eventos, cancelamento e propriedade

Eventos internos carregam geração, work ID, execution ID e sequência. O planejador rejeita identidades antigas antes de atualizar estado. Eventos externos preservam sessão, sequência e live request ID existentes.

Cancelar invalida a geração, impede novos despachos e aguarda limpeza. `stop` enviado não significa worker livre. Cada stream continua sob um único leitor até `bestmove` ou término confirmado; timeout ou protocolo inválido exigem teardown. Busca ao vivo interrompida/falha mata e aguarda o processo antes de reutilização, conforme o contrato existente.

O owner retém o permit exclusivo até terminar todos os processos. Fechamento idempotente drena operações de banco e o save de histórico que a sessão possui. Cancelamento não aparece como erro de análise, e falha de limpeza não mascara a falha original.

Limitar filas e coalescer somente progresso descartável. Baseline, conclusão e erro precisam de entrega confiável. Nunca bloquear leitura de stdout porque a UI ou uma fila de progresso está lenta. A extração não muda a política atual de navegação; preemptar revisão automática para análise ao vivo seria uma decisão de produto separada.

## Validação e critérios de adoção

Usar três níveis complementares:

1. **Replay determinístico:** mesmas baselines, alvos e respostas simuladas, variando ordem e política. Verifica quotas, fairness, orçamento e ausência de duplicação. Não mede qualidade do Stockfish.
2. **Stockfish com alvos congelados:** mesmas posições, requisitos finais, binário e recursos; isolar a execução de refinamentos das diferenças na seleção. Manter o custo de obtenção das baselines explícito; esse ensaio não representa sozinho a revisão completa.
3. **Partida completa real:** mesmas partidas, binário, recursos e configuração de contexto/cache; medir seleção efetivamente produzida e comparar resultados com referência mais cara. Uma referência também pode errar; investigar divergências, mates e sacrifícios individualmente. Avaliar recall de erros graves, concordância de classificação e perda associada ao lance escolhido; separar mates e mudanças de resultado de diferenças pequenas em centipawns.

Executar comparações separadas para fila atual, afinidade, continuidade sem espera e continuidade com espera limitada. Separar custo de contexto/cache, custo de MultiPV e efeito do scheduler; não atribuir todos a continuidade.

O corpus precisa incluir FEN inicial, repetição, cinquenta lances, promoções, mates, táticas, finais e partidas longas, com cache frio/parcial/quente quando aplicável. Medir tempo até primeira baseline e cobertura completa, parede da partida, CPU, memória agregada, navegação/cancelamento p95, trabalho especulativo descartado, reinícios e conjunto final de alvos. Profundidade e nós são diagnósticos, não o critério de qualidade.

Antes de executar a comparação final, registrar em um manifesto: corpus e hash, binário/configuração, repetições e ordem, classe de máquina, métricas principais, margem de não inferioridade de qualidade e limites de memória/latência. Os valores ainda precisam ser escolhidos; este RFC não os apresenta como evidência existente.

Gates absolutos: nenhuma perda de alvo obrigatório; nenhuma baseline alterada pelo refinamento; nenhuma contaminação de cache; nenhuma execução duplicada; limpeza completa e eventos obsoletos rejeitados. Gates de desempenho dependem dos limiares previamente registrados. Ganho só em amostra pequena ou somente em profundidade não aprova produção.

## Sequência de implementação proposta

| Etapa | Entrega | Condição de saída |
| --- | --- | --- |
| 1. Caracterização | Fixtures e traces da fila atual; ledger mínimo | Comportamento atual reproduzível, inclusive afinidade local |
| 2. Contratos de resultado e contexto | Tipos internos, assembler testável, política explícita de cache | Casos históricos e linhas parciais não geram equivalência falsa |
| 3. Extração pequena do planejador | Estado/seleção separados do loop assíncrono, política atual preservada | Replay equivalente e suíte existente passando |
| 4. Driver experimental | Eventos/checkpoints acima de `EnginePort`, sem quebrar `evaluate` atual | Ordens adversas, cancelamento e sidecar real verificados |
| 5. Janela experimental | Política opcional e reconciliação Rust compartilhada | Comparação por partida com orçamento auditável |
| 6. Decisão de adoção | Relatório contra gates, fallback exercitado | Benefício consistente sem violar qualidade/custo definidos |

Não condicionar a correção de contexto a um ganho de continuidade; são problemas independentes. Também não migrar cache nem mudar protocolo público só para viabilizar o primeiro ensaio.

Quando houver mudanças de implementação: executar `bun run lint && bun run typecheck && bun run test` em `app/` e `cargo test` em `app/src-tauri/`, incluindo a integração real com o sidecar disponível. Benchmark fake-engine continua restrito ao custo da pipeline Rust.

## Decisões ainda abertas

- Qual política de baseline incompleta usar sem alterar o contrato nominal dos perfis?
- Qual classe de contexto permite reutilização FEN-only comprovadamente segura, e quando criar cache contextual?
- Quais margens quantitativas de qualidade, memória e responsividade serão exigidas por classe de máquina?
- Há benefício suficiente em manter MultiPV de triagem durante refinamento para justificar continuidade?
- O produto deseja um teto global que possa terminar com cobertura incompleta? Essa escolha é independente do primeiro refactor.
