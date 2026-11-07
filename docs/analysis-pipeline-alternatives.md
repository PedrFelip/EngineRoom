# Alternativas para a pipeline de análise

As propostas estão separadas por decisão arquitetural. Nenhuma foi adotada ou demonstrada superior por benchmarks.

## Alternativa A — Planejador por posições e etapas

[Documento completo](analysis-pipeline-design.md).

Organiza trabalhos por posição, com baseline congelada, seleção de refinamentos e reconciliação global. Separa planejamento puro da execução assíncrona. A proposta atual começa preservando a política existente, mas essa separação também pode ser implementada do zero.

Escolher quando o objetivo principal for melhorar controle, testes e agendamento da revisão em duas etapas. O limite é continuar orientando o trabalho por posições e cobertura de buscas.

## Alternativa B — Decisões e evidências

[Documento completo](analysis-greenfield-architecture.md) e [plano de implementação](analysis-b-implementation-plan.md).

Organiza trabalhos pelas perguntas necessárias para julgar cada lance. Buscas produzem evidências compartilhadas; conclusões evoluem com revisões explícitas. Triagem e refinamento são políticas possíveis, não etapas obrigatórias da arquitetura.

Escolher quando o objetivo incluir verificação específica do lance jogado, explicações e resolução de avaliações conflitantes. O custo principal é definir critérios operacionais de evidência suficiente.

Esta é a direção recomendada para um desenho do zero, ainda sujeita a validação de uma fatia vertical.

## Opção de execução C — Janela com continuidade

[Documento completo](analysis-window-pipeline.md).

Organiza a admissão de posições próximas e permite promover buscas ainda ativas. É uma alternativa de execução às buscas separadas; pode ser combinada com A ou B. Não constitui, sozinha, um terceiro modelo de domínio.

Escolher somente se a comparação demonstrar benefício líquido de qualidade e custo. Manter a busca ativa também consome recursos e conserva seu MultiPV.

## Combinações possíveis

- A com buscas separadas: evolução mais próxima do comportamento atual.
- A com janela contínua: experimenta continuidade mantendo seleção por etapas.
- B com buscas separadas: MVP recomendado para validar o novo modelo de domínio.
- B com janela contínua: otimização posterior, se houver evidência favorável.

Afinidade, quantidade de workers e divisão de Threads/Hash são decisões de execução, não novas arquiteturas. Um fork do Stockfish permanece uma possibilidade adiada, sem proposta executável ou benefício demonstrado.

Os [requisitos](analysis-pipeline-requirements.md) são a referência comum. O [documento de continuidade](local-analysis-architecture.md) registra hipóteses, limites e ensaios anteriores.
