# Alternativa de execução: janela deslizante com continuidade

Status: hipótese experimental; não implementada na pipeline de produção nem aprovada por benchmarks. Os harnesses existentes verificam casos limitados, não uma revisão completa.

Esta alternativa muda a política de execução. Pode ser aplicada ao [planejador por posições](analysis-pipeline-design.md) ou ao [modelo por decisões e evidências](analysis-greenfield-architecture.md). Não exige escolher outro modelo de domínio.

## Objetivo e fluxo

Admitir um conjunto limitado de posições próximas, obter baselines e aproveitar uma busca ainda ativa quando a informação dos vizinhos justificar refinamento antes de encerrá-la.

Fluxo: admitir posições → executar buscas → congelar baselines → decidir promoção local → continuar ou encerrar → avançar janela → reconciliar seleção global → executar déficits.

## Política proposta

Limitar separadamente trabalhos na janela, processos residentes e buscas simultâneas. O número de processos deriva do orçamento de recursos; a quantidade de trabalhos não implica igual quantidade de processos.

1. Admitir posições em ordem e priorizar dependências vizinhas necessárias à decisão local.
2. Ao obter baseline coerente, congelá-la e consultar a mesma seleção hard do Rust.
3. Continuar no mesmo `go` somente se a decisão chegar antes de enviar `stop`, houver autorização de orçamento e a cobertura fixa daquela execução servir ao alvo.
4. Inicialmente usar espera adicional zero. Uma variante com espera limitada deve declarar o limite e cobrar toda busca enquanto espera como custo especulativo.
5. Sem promoção autorizada, parar, aguardar término e liberar o worker. Uma decisão tardia agenda novo `go`; nunca registrar isso como continuidade.
6. Reservar progresso da triagem. Com um worker e triagem pendente, não reter a execução só para esperar vizinhos.
7. Após cobertura completa de baselines, reconciliar globalmente e executar os déficits com buscas separadas quando necessário.

MultiPV permanece fixo por execução. Continuidade com MultiPV 3/5 e reinício com MultiPV 1/2 são variantes distintas: maior número de linhas não prova melhor qualidade para o mesmo custo. Janela e espera serão parâmetros do experimento, não novos defaults de perfil.

## Vantagens possíveis

- Preservar uma busca ativa em promoções descobertas cedo.
- Aproximar temporalmente as avaliações de posições vizinhas.
- Limitar residência e especulação sem manter um processo por posição da partida.

Essas vantagens são hipóteses, não ganhos de qualidade ou velocidade demonstrados.

## Custos e limites

- Busca ativa esperando vizinhos consome CPU e ocupa capacidade.
- MultiPV fixo de triagem pode tornar o refinamento mais caro que reiniciar com menos linhas.
- Decisões tardias exigem reinício; continuidade não pode ser garantida para todos os alvos.
- A seleção local não substitui reconciliação global nem quotas finais.
- Janela, tempo de espera e orçamento especulativo acrescentam parâmetros ao scheduler.

## Validação necessária

Comparar com buscas separadas usando os mesmos alvos, recursos e orçamento total; depois comparar partidas completas com seleção real. Medir qualidade, CPU, memória agregada, parede, cancelamento, reinícios e trabalho descartado.

O [experimento de continuidade](local-analysis-architecture.md) contém os limites UCI e o histórico das evidências. Afinidade oportunista está descrita [separadamente](triage-refinement-affinity.md): ela prefere um worker livre, sem manter uma busca ativa nem esperar seu processo de origem.

## Quando escolher

Somente se o ganho medido de continuidade superar o custo de MultiPV, espera e complexidade. Sem esse resultado, manter buscas separadas é uma escolha válida em qualquer uma das arquiteturas de domínio.
