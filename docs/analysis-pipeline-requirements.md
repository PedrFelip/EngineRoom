# O que considerar em uma pipeline de análise

Este documento reúne os requisitos e as decisões para evoluir a análise local do EngineRoom. Complementa a [proposta de arquitetura local](local-analysis-architecture.md) e o [desenho de afinidade entre triagem e refinamento](triage-refinement-affinity.md). É uma lista de critérios para orientar o projeto; não significa que todos já estejam implementados.

Qualidade da análise, aproveitamento da pesquisa e custo no computador do usuário precisam ser avaliados juntos. Preservar uma busca só vale a pena se produzir resultados melhores ou reduzir o custo da revisão.

## 1. Resultado esperado

- Detectar erros, oportunidades perdidas e decisões críticas.
- Produzir avaliações, melhores lances e variantes confiáveis.
- Entregar resultados preliminares rapidamente.
- Melhorar a análise conforme o orçamento disponível.
- Manter a interface e o computador utilizáveis durante a revisão.
- Definir métricas para esses objetivos: profundidade e quantidade de nós, isoladamente, não demonstram qualidade.

## 2. Unidade de trabalho

| Entidade | Responsabilidade |
| --- | --- |
| Partida | Contexto, sequência de posições, progresso e resultado final |
| Posição | FEN e histórico necessário para interpretação correta |
| Trabalho de análise | Baseline, prioridade, cobertura exigida, orçamento e resultados |
| Execução de busca | Uma chamada `go`, com configuração e ciclo de vida próprios |
| Processo Stockfish | Estado residente, hash, heurísticas e busca em execução |
| Threads | Recursos que cooperam dentro da busca |

Um trabalho pode sobreviver a várias execuções. Isso não significa que o estado interno da pesquisa sobreviveu. Essa diferença deve ser registrada explicitamente.

## 3. Correção das posições

- Validar o PGN e respeitar posições iniciais por FEN.
- Tratar posições terminais sem buscas desnecessárias.
- Preservar a perspectiva correta das avaliações.
- Considerar roque, en passant, regra dos cinquenta lances e repetição.
- Avaliar quando a FEN é suficiente e quando precisamos enviar o histórico de lances ao motor.
- Evitar deduplicar posições que parecem iguais, mas têm contexto relevante diferente.

## 4. Função da triagem

- Definir a informação mínima necessária para selecionar refinamentos.
- Escolher tempo e quantidade de variantes adequados.
- Detectar baseline incompleta ou instável.
- Definir quando congelar a avaliação preliminar.
- Tratar informações que chegam depois do prazo.
- Avaliar orçamento fixo versus parada por algum critério de confiança.

A baseline usada para selecionar posições deve permanecer imutável, para o próprio refinamento não alterar retroativamente sua justificativa.

## 5. Seleção dos refinamentos

- Comparar avaliações antes e depois do lance.
- Detectar perdas, viradas, mates, promoções e instabilidade.
- Diferenciar candidatos obrigatórios dos secundários.
- Considerar posições vizinhas e contexto de sacrifícios.
- Deduplicar alvos sobrepostos e combinar seus requisitos.
- Permitir descoberta antecipada sem perder a seleção global final.
- Preservar quotas e impedir que refinamentos monopolizem os recursos.
- Decidir se refinamos a posição antes do lance, depois dele ou ambas, conforme o objetivo.

## 6. Continuidade da pesquisa

Escolher quais níveis de reaproveitamento suportar:

- Resultado persistido que dispensa uma busca.
- Nova busca no mesmo processo, aproveitando memória residual.
- Mesma busca ativa, com triagem e refinamento como marcos.
- Eventual pausa cooperativa no motor, caso um fork se justifique.

Para busca contínua, definir quanto tempo esperar pelas posições vizinhas, quando encerrar uma pesquisa e quando aceitar um reinício. Com recursos limitados, não é possível garantir que todas as buscas permaneçam vivas até a seleção global.

## 7. Parâmetros do Stockfish

- Threads por busca.
- Hash por processo.
- MultiPV na triagem e no refinamento.
- Limites por tempo, profundidade ou nós.
- Configurações e versão do motor.
- Comportamento em posições simples, táticas e finais.

MultiPV é uma decisão central: manter mais linhas favorece a cobertura, mas divide esforço de pesquisa. No experimento de continuidade, ele permanece fixo durante o mesmo `go`.

## 8. Orçamentos

Separar:

- Tempo nominal autorizado.
- Tempo efetivo de busca.
- Tempo de CPU.
- Tempo de parede.
- Espera por decisões.
- Inicialização, configuração e encerramento.
- Excesso de prazo até o motor responder ao `stop`.

Definir orçamento por posição e por partida, limites de refinamento antecipado e regras para aumentar a cobertura. Não somar buscas separadas e tratá-las automaticamente como uma busca contínua equivalente.

Os orçamentos atuais dos perfis permanecem em `app/src-tauri/src/review/profiles.json`, sua fonte de verdade compartilhada. Experimentos devem declarar qualquer diferença em relação a esses contratos.

## 9. Agendamento

- Quantas posições podem estar residentes e quantas podem pesquisar simultaneamente?
- Como avançar a janela pela partida?
- Como garantir que as posições vizinhas cheguem a tempo?
- Quando priorizar triagem, refinamento ou navegação?
- Como evitar ociosidade, starvation e filas atrás de trabalhos lentos?
- Como lidar com resultados fora de ordem?
- Quando favorecer afinidade e quando redistribuir trabalho?

O número de processos deve resultar dessa política e dos recursos disponíveis; não precisa ser a abstração principal da arquitetura.

## 10. Ambiente local

- CPU realmente disponível, incluindo limites do sistema.
- RAM disponível e pressão de memória.
- Memória total dos processos, além do Hash.
- Custo de múltiplas instâncias e redes NNUE.
- Diferenças entre Linux e Windows.
- Aquecimento, throttling e uso em bateria.
- Concorrência com navegador, jogos e outros aplicativos.
- Responsividade da interface e latência de cancelamento.

Adotar comportamento conservador em máquinas pequenas e medir antes de escolher a distribuição de recursos.

## 11. Resultados intermediários e finais

- Checkpoints com linhas completas e profundidade coerente.
- Tratamento explícito de scores com bounds.
- Identidade da posição, execução e geração.
- Proveniência: cache, triagem, continuidade ou reinício.
- Regra para aceitar refinamento menos profundo.
- Preservação das linhas preliminares.
- Publicação conforme os resultados terminam.

Comunicar progresso útil sem apresentar uma avaliação parcial como definitiva.

## 12. Cache e histórico

- Compatibilidade por profundidade, tempo e MultiPV.
- Contexto necessário para identificar corretamente uma posição.
- Proveniência e eventual compatibilidade entre versões e configurações.
- Consulta em lote e gravação incremental.
- Cache parcial, resultados corrompidos e falhas reais de armazenamento.
- Compatibilidade das revisões salvas.
- Separação entre cache de resultados e memória interna do motor.

Checkpoints experimentais não devem entrar no cache existente sem uma semântica de cobertura bem definida. Preservar os contratos atuais de cobertura, migrações, flush e propagação de erros descritos em `AGENTS.md`.

## 13. Cancelamento e recuperação

- Cancelar em qualquer etapa.
- Rejeitar eventos antigos.
- Garantir que a navegação mais recente prevaleça.
- Tratar timeout, saída inesperada e protocolo inválido.
- Encerrar e aguardar todos os processos possuídos.
- Manter a exclusividade até terminar a limpeza.
- Preservar o erro original quando a recuperação também falhar.
- Fechar o aplicativo sem deixar processos ou operações pendentes.

Essas regras precisam fazer parte do desenho desde o início.

## 14. Validação e observabilidade

- Testes determinísticos com engines falsas.
- Integração com Stockfish real.
- Corpus com aberturas, táticas, finais, mates e partidas longas.
- Casos com cache vazio, parcial e completo.
- Diferentes classes de máquina e divisões de recursos.
- Comparações com orçamento e cobertura equivalentes.
- Referência mais cara para investigar qualidade e divergências.
- Registro de continuações, reinícios, trabalho descartado, CPU, memória e latências.
- Repetições e dispersão dos resultados.

Separar claramente experimentos por posição, janela e partida completa. O benchmark Rust com engine falsa não mede desempenho de Stockfish, IPC ou renderização.

## 15. Evolução e manutenção

- Preservar os contratos de engine e repositório injetáveis.
- Manter decisões de análise no Rust.
- Introduzir mudanças experimentais com fallback.
- Evitar duplicar configurações dos perfis.
- Avaliar custo de manutenção antes de incorporar ou modificar o Stockfish.
- Definir critérios de adoção antes de analisar os resultados.

## Decisões que orientam o próximo desenho

1. O que constitui um trabalho e qual estado pertence a ele?
2. Quando congelamos a triagem e qual cobertura mínima exigimos?
3. Como selecionamos e priorizamos refinamentos locais e globais?
4. Quanto estado mantemos vivo sob limites de CPU e memória?
5. Qual custo total autorizamos e como verificamos seu cumprimento?

Essas escolhas devem orientar o scheduler e a distribuição de threads.
