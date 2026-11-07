# Contrato experimental B (versão 1)

`Planner::new(pgn, Config)` valida o PGN e cria uma geração; `next_request(now_ms)` devolve no máximo uma execução pendente (`Dispatch`, com geração, execução e `Request`). O relógio é fornecido pelo chamador em milissegundos monotônicos; não representa CPU. `accept(generation, execution_id, evidence, now_ms)` aceita somente a execução ativa e evidência que satisfaça integralmente seu pedido. `reject` encerra uma execução sem resultado utilizável; `snapshot()` exporta estado serializável, incluindo evidências, conclusões, pendências e ledger. O chamador deve encerrar o processo antes de rejeitar ou substituir uma execução. Não há persistência nem lease nesta API.

O manifesto `Config` fixa assinatura da engine, profundidade e MultiPV do screening, profundidade de verificação, limiar em centipawns e tentativas máximas. É experimental e independente de `profiles.json`. Cada posição contextual tem uma busca de screening irrestrita; posições vizinhas compartilham a mesma evidência. Melhor lance é a primeira linha exata; valor jogado é o score irrestrito do filho, invertido para o jogador anterior. Conclusões citam ambas as evidências, ou um resultado terminal de regras sem execução inventada. Uma posição terminal não é buscada. Screening é imutável; verificações geram revisões separadas. Diferença de centipawns acima do limiar ou qualquer mate abre verificação, após a cobertura inicial. A verificação busca novamente ambas as posições não terminais em profundidade maior; conflito persistente ou falta de evidência esgota tentativas e termina unresolved. Não se faz equivalência numérica entre mate e centipawns nem comparação de distância de mate entre pai e filho.

A política `b-experimental-2` distingue perda crítica de conflito de estimativas:
quando o valor jogado supera o melhor por pelo menos o limiar, duas buscas
independentes são inconsistentes e geram uma obrigação de verificação, sem
classificar o lance como erro crítico. O mesmo limite de tentativas se aplica;
se o conflito persistir, a decisão fica pendente com motivo específico. Scores
de mate seguem a regra própria, sem conversão para centipawns. A versão nova no
contexto impede reutilizar evidência contextual gravada para a política anterior;
snapshots antigos continuam legíveis com `conflict` ausente como `false`.

Scores são relativos ao lado a jogar na posição avaliada; inversão troca sinal e bounds, e mate zero permanece zero. Empates por três repetições ou 100 halfmoves já ocorridos são reivindicáveis, sem encerrar a busca; cinco repetições ou 150 halfmoves são automáticos, com xeque-mate prioritário. Reivindicações por lance anunciado e histórico anterior a uma FEN não são inferidos. O contexto preserva raiz e prefixo completo; assinatura exige digest SHA-256 hexadecimal do executável e opções canônicas, fornecidos pelo executor, e compatibilidade exige igualdade exata. Um conjunto de linhas de uma execução precisa satisfazer toda a cobertura, incluindo profundidade comum, raízes distintas e PVs legais. Limite de MultiPV é o número de lances legais.

O ledger registra profundidade autorizada, reserva ativa e tempo de parede observado; CPU e memória não medidas são `null`, nunca zero presumido. Identidades de geração e execução impedem eventos obsoletos. Falha técnica e cancelamento devem ser tratados pelo executor; rejeição de resultado inutilizável é registrada como pendência, não como resultado de xadrez.

`Planner` implementa `runner::Plan`; despacho e conclusão são distintos, e `accept` e `reject` devolvem `Result<()>`. O executor propaga rejeições e liquida reservas após encerrar o processo. A geração deve ser única por execução do planejador, sob responsabilidade do chamador.

A evidência de verificação mais recente de cada posição é compartilhada pelas duas decisões adjacentes. Ao completar uma tentativa (ou rejeitá-la após uma busca parcial), o planejador publica revisões para todas as conclusões afetadas, com IDs das evidências efetivamente usadas; o screening original permanece imutável. A conclusão mais recente pode abrir uma obrigação em uma decisão já examinada. Cada decisão dispõe de no máximo `max_attempts` tentativas próprias, inclusive descobertas tardias; ao esgotar o orçamento, uma conclusão ainda crítica termina como pendência. A cobertura inicial precede todas as verificações. Uma rejeição de verificação esgota o orçamento daquela decisão e registra o motivo; evidências aceitas anteriormente continuam disponíveis para vizinhas.

### Contrato do runner e adaptação da CLI

`runner::Plan::next` retorna `Result<Next>`: `Dispatch` autoriza uma execução e `Complete` afirma conclusão; uma reserva ativa ou estado incompleto sem despacho é erro. `accept` e `reject` retornam `Result<()>` e devem informar rejeições. Após adquirir o engine, o runner aguarda shutdown antes de retornar, inclusive em falha ou cancelamento; uma reserva ativa é rejeitada após o shutdown, e falha ao liquidá-la é reportada explicitamente. Adaptadores externos, inclusive `examples/analysis_b.rs`, precisam implementar essas assinaturas e propagar os erros, sem `assert!`.

O driver considera cada slot 1 como início de um ciclo MultiPV ordenado na sua profundidade; slots fora de ordem invalidam o ciclo parcial. Um ciclo completo válido anterior permanece disponível até outro completo válido substituí-lo.

O chamador deve sinalizar `Cancellation` e aguardar a future do runner até o fim; descartá-la ou abortá-la não garante shutdown aguardado. A integração futura no aplicativo precisa de uma task proprietária cujo encerramento seja aguardado.
