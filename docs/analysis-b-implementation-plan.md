# Plano de implementação — alternativa B

Status: implementação parcial experimental. A primeira entrega funcional ainda não está pronta; consulte o registro ao final.

Referências: [arquitetura B](analysis-greenfield-architecture.md), [requisitos](analysis-pipeline-requirements.md) e [alternativas](analysis-pipeline-alternatives.md).

## Objetivo e primeira entrega

Construir uma revisão orientada a decisões: cada lance possui perguntas; buscas produzem evidências compartilhadas; classificações são conclusões versionadas.

A primeira entrega funcional é uma **partida completa analisada por um runner Rust experimental**, com cobertura inicial, verificação limitada dos casos críticos e relatório exportado. Usar Stockfish real, sem depender de nova UI ou do SQLite de produção. Uma engine falsa cobre as mesmas transições de forma determinística.

A integração no app vem depois de verificar correção, custo e utilidade dessa entrega. Adotar B no planejamento não significa que já existe evidência para substituir a implementação atual.

## Como usar este plano

Cada ID é uma unidade de trabalho que pode virar issue ou PR. Atualizar seu estado e registrar links, evidência de validação e pendências ao concluir. Usar `pendente`, `em andamento`, `bloqueada` ou `concluída`; não marcar concluída apenas porque os tipos compilam.

O escopo de cada tarefa deve caber em uma revisão independente. Se houver alterações grandes, dividir mantendo o ID como agrupador. PRs de implementação seguem o fluxo do repositório para `develop`.

## Decisões iniciais

- Implementar o núcleo em Rust, com tipos específicos de xadrez e funções puras. Arrays e índices de dependências bastam; nenhum framework genérico de DAG.
- Usar inicialmente buscas separadas, irrestritas, na posição anterior e no filho. Busca restrita, continuidade, WDL e sizing adaptativo ficam para depois.
- O primeiro runner opera com um worker; concorrência limitada entra após a fatia vertical funcionar.
- Manter evidências em memória e relatórios experimentais isolados até definir persistência contextual.
- Começar com contrato de cobertura e tentativas finitas. O término pode conter perguntas `unresolved`; isso é diferente de falha técnica e de cancelamento.
- Reutilizar `EngineFactory`, `EnginePort`, parsing de PGN e scoring quando seus contratos servirem. Não fazer B depender da extração de A.
- Perfis continuam vindo de `profiles.json`. Qualquer política experimental diferente deve declarar valores e versão num manifesto separado, sem alterar silenciosamente os perfis existentes.
- Durante coexistência, preservar comandos públicos, cache legado, revisões salvas e propriedade dos recursos descritos em `AGENTS.md`.

## Mapa das entregas

| Marco | Resultado observável | Tarefas |
| --- | --- | --- |
| M0 — Contratos | Semântica mínima escrita e fixtures escolhidas | B-01 |
| M1 — Núcleo determinístico | Partida inteira revisada com engine falsa | B-02 a B-05 |
| M2 — Fatia vertical real | Mesmo fluxo usando Stockfish, com relatório e cleanup | B-06 a B-08 |
| M3 — Avaliação | Comparação reproduzível e decisão de continuidade do projeto | B-09 |
| M4 — Persistência e sessão | Cache contextual, histórico e ciclo de vida integrados sob opção experimental | B-10 a B-12 |
| M5 — Produto | UI compreende progresso, revisões e pendências; fallback exercitado | B-13 a B-14 |

## Backlog por tarefa

### B-01 — Fechar o contrato mínimo de análise

Estado: pendente. Dependências: nenhuma.

- [ ] Definir quais evidências satisfazem `BestMove` e `PlayedMoveValue` na primeira versão.
- [ ] Definir gatilhos de `VerifyCritical`, seu orçamento, limite de tentativas e saída `unresolved`.
- [ ] Definir política versionada para empate reivindicável, repetição e histórico desconhecido antes da FEN inicial.
- [ ] Fixar assinatura conservadora da engine: digest do binário/build e opções configuradas; documentar compatibilidade por contexto, escopo e cobertura.
- [ ] Definir estados de conclusão: preliminar, protocolo cumprido, pergunta não resolvida; separar cancelamento e falha.
- [ ] Selecionar fixtures com tática, mate, promoção, repetição, cinquenta lances, FEN inicial e partida longa.

Pronto quando: exemplos concretos de entrada/evidência/conclusão permitem implementar as regras sem escolher semântica incidentalmente no código. Nenhum gatilho é apresentado como probabilidade de acerto.

### B-02 — Contexto e decisões por lance

Estado: pendente. Dependências: B-01.

- [ ] Injetar snapshots de relógio nos testes do planejador/runner, sem consultar relógio real no núcleo puro.
- [ ] Criar módulo experimental isolado, por exemplo `review/decision/`, com API interna mínima.
- [ ] Construir `PositionContext` a partir do PGN validado, preservando raiz e prefixo de lances.
- [ ] Construir `DecisionCase` com contexto anterior, lance jogado e contexto posterior.
- [ ] Identificar contextos e gerações sem deduplicar apenas por FEN; representar limite do histórico conhecido.
- [ ] Tratar posições terminais segundo a política definida, sem busca desnecessária.

Pronto quando: uma partida de N lances produz N decisões, dependências corretas e contextos distintos nos casos históricos relevantes. Fixtures cobrem FEN inicial e perspectiva.

### B-03 — Evidência, cobertura e conclusões

Estado: pendente. Dependências: B-01 e B-02.

- [ ] Tipar scores, mates, bounds, perspectiva, escopo e identidade da engine.
- [ ] Separar `EvidenceRequest`, execução, evidência imutável e conclusão versionada.
- [ ] Implementar predicado de satisfação por evidência individual; não combinar máximos de execuções diferentes.
- [ ] Manter screening imutável e referências às evidências em cada revisão da conclusão.
- [ ] Representar origem terminal, cache ou engine sem inventar uma execução nos casos sem busca.

Pronto quando: testes rejeitam contextos e escopos incompatíveis, bounds insuficientes e cobertura fictícia; uma nova conclusão não altera evidências anteriores.

### B-04 — Planejador de cobertura inicial

Estado: pendente. Dependências: B-02 e B-03.

- [ ] Gerar perguntas mínimas para todos os lances e compartilhar evidências entre decisões vizinhas.
- [ ] Implementar transições puras dirigidas por eventos e ações de busca/publicação.
- [ ] Evitar solicitações ativas duplicadas; rejeitar eventos de outra geração ou execução.
- [ ] Publicar conclusão preliminar quando suas dependências estiverem disponíveis, sem esperar a partida inteira.

Pronto quando: engine falsa cobre uma partida completa em ordens adversas, sem consultas redundantes para o mesmo pedido compatível, e produz saída determinística para a mesma sequência de eventos.

### B-05 — Verificação crítica e término finito

Estado: pendente. Dependências: B-01, B-03 e B-04.

- [ ] Abrir verificações críticas com motivo e referências às evidências que as originaram.
- [ ] Gerar novas conclusões sem reescrever o screening; admitir descoberta posterior explicitamente.
- [ ] Garantir progresso da cobertura com um worker, com política determinística e limite de verificações antecipadas.
- [ ] Limitar tentativas e expansão de obrigações; registrar por que uma pergunta terminou não resolvida.
- [ ] Contabilizar autorizações, reservas e gasto por execução; representar medições indisponíveis como ausentes.

Pronto quando: conflitos persistentes não geram loop infinito; toda pergunta admitida termina segundo sua política; o relatório distingue cobertura, verificações executadas e perguntas resolvidas. Este marco conclui M1.

### B-06 — Driver de busca contextual

Estado: pendente. Dependências: B-02 e B-03. Pode avançar em paralelo a B-04/B-05 após estabilizar os tipos.

- [ ] Implementar driver sobre `EnginePort`, mantendo um leitor por processo e um `go` ativo.
- [ ] Enviar posição raiz e histórico conhecido; fixar opções durante a execução.
- [ ] Montar resultados coerentes: profundidade comum, slots completos, PVs legais e primeiras jogadas distintas.
- [ ] Conservar último conjunto completo ao receber atualização parcial; tratar ausência de resultado utilizável explicitamente.
- [ ] Registrar término, limites, tempo efetivo e overshoot sem equiparar orçamento autorizado a cobertura observada.
- [ ] Preservar limpeza após timeout, falha e cancelamento, incluindo encerramento confirmado do processo.

Pronto quando: testes de protocolo cobrem bounds, MultiPV parcial, mate, saída inesperada e cancelamento; integração real confirma contexto enviado e término de processos. Não alterar a semântica do caminho atual incidentalmente.

### B-07 — Runner experimental de partida completa

Estado: pendente. Dependências: B-05 e B-06.

- [ ] Conectar planejador, driver e store em memória com um worker.
- [ ] Disponibilizar ponto de entrada reproduzível, como exemplo/binário Rust, para PGN e manifesto.
- [ ] Adquirir e manter lease pelo mecanismo existente quando executado dentro do app; garantir cleanup no runner standalone.
- [ ] Exportar manifesto, evidências, conclusões, pendências e ledger em formato versionado.
- [ ] Garantir que o runner não abra nem escreva o banco de produção.

Pronto quando: comando documentado analisa uma partida completa com sidecar real e exporta resultado auditável; erro ou cancelamento não deixa processo vivo. Esta é a primeira entrega funcional para avaliação manual.

### B-08 — Concorrência limitada e observabilidade

Estado: pendente. Dependências: B-07.

- [ ] Admitir workers dentro de um orçamento total de Threads/Hash, sem processo por decisão.
- [ ] Aplicar despacho determinístico, ausência de duplicação e progresso da cobertura sob conclusões fora de ordem.
- [ ] Limitar filas e coalescer progresso sem bloquear stdout ou perder conclusões.
- [ ] Medir parede, CPU e memória agregadas onde suportadas, declarando limitações por plataforma.
- [ ] Registrar causas das buscas, trabalho descartado e latências de cancelamento; não alegar CPU medida a partir de `movetime`.

Pronto quando: cenários com um e vários workers preservam invariantes; cancelamento drena todos os processos. Métricas ausentes não aparecem como zero. Este marco conclui M2.

### B-09 — Comparar B com a pipeline atual

Estado: pendente. Dependências: B-07; comparação de concorrência depende também de B-08.

- [ ] Caracterizar dispersão da baseline e registrar métricas principais, margens e limites antes da comparação final.
- [ ] Executar replay determinístico, Stockfish com obrigações congeladas e partidas completas com seleção própria.
- [ ] Separar efeito de contexto, cache, seleção e concorrência; não atribuir toda diferença ao modelo B.
- [ ] Comparar qualidade com referência mais cara e investigar divergências críticas; reportar custo e pendências.
- [ ] Registrar decisão: avançar, revisar política ou interromper a substituição, com evidências e limitações.

Pronto quando: comandos, corpus, binário, configurações, repetições e resultados podem ser reproduzidos. Avançar para integração exige correção/lifecycle completos e benefício útil dentro dos critérios previamente registrados. Nenhum ganho numérico é presumido neste plano.

### B-10 — Cache contextual e store persistente

Estado: pendente. Dependências: B-03 e decisão favorável em B-09.

- [ ] Definir schema/namespace versionado de evidências com contexto verificável, engine, opções, escopo e cobertura.
- [ ] Implementar consultas em lote e gravações incrementais; preservar distinção entre corrupção e erro real de I/O.
- [ ] Definir retenção e semântica de compatibilidade, sem promover entradas FEN-only a evidências históricas.
- [ ] Criar migrações idempotentes e testar coexistência com o schema e as consultas atuais.

Pronto quando: round-trip preserva semântica; casos incompatíveis não dão hit; falha de gravação preserva erro original. Não alterar PK/SQL legado silenciosamente.

### B-11 — Histórico e compatibilidade de revisões

Estado: pendente. Dependências: B-10.

- [ ] Persistir snapshot suficiente de conclusões e evidências para sobreviver à limpeza do cache.
- [ ] Reabrir revisões B e conservar leitura/normalização das revisões antigas.
- [ ] Versionar classificador e formato; manter PGN como fonte dos metadados.
- [ ] Definir exclusão de histórico e limpeza de evidências sem referências quebradas.

Pronto quando: reabrir uma revisão não exige engine; limpar cache não destrói histórico; fixtures antigas continuam legíveis.

### B-12 — Integração de sessão e navegação

Estado: pendente. Dependências: B-08, B-10 e B-11.

- [ ] Integrar o estado de B ao owner de sessão, com único escritor e execução fora do loop de estado.
- [ ] Compartilhar exclusividade com outras sessões e probes de Settings; manter permit até teardown completo.
- [ ] Serializar navegação pelo request ID mais recente e rejeitar eventos obsoletos antes de publicá-los.
- [ ] Fechar de forma idempotente, drenando processos, operações de banco e save de histórico possuído.
- [ ] Expor B por opção experimental interna com fallback explícito para novas revisões; não converter uma execução parcialmente concluída em A silenciosamente.

Pronto quando: testes adversos cobrem aquisição interrompida, navegação rápida, fechamento e erro de persistência. Cancelamento não vira erro de análise. Este marco conclui M4.

### B-13 — Protocolo e interface progressiva

Estado: pendente. Dependências: B-12; desenho do contrato pode ocorrer antes.

- [ ] Definir projeção pública de conclusões, revisões, progresso e pendências sem expor stdout ou detalhes de engine.
- [ ] Validar IPC com Schema e Effect; preservar estado de navegação separado do I/O.
- [ ] Mostrar avaliação preliminar, atualização de classificação e perguntas não resolvidas de modo compreensível.
- [ ] Verificar resumo/acurácia, navegação para erros e reabertura após revisões de conclusão.
- [ ] Manter acesso ao caminho atual durante avaliação experimental.

Pronto quando: eventos antigos não regredem a UI; concluir o protocolo com pendências não é exibido como erro técnico ou análise plenamente resolvida.

### B-14 — Validar adoção e operação

Estado: pendente. Dependências: B-09 e B-13.

- [ ] Repetir comparação de qualidade/custo incluindo IPC, UI e persistência em classes de máquina representativas.
- [ ] Exercitar fallback, instalação existente, migrações e dados antigos.
- [ ] Verificar licenças, recursos distribuídos e documentação de uso quando afetados.
- [ ] Registrar decisão de ativação por perfil/capacidade ou de permanência experimental.

Pronto quando: gates registrados foram cumpridos; limitações e plano de fallback estão documentados. Remover o caminho antigo é trabalho posterior explícito, condicionado à adoção.

## Divisão de frentes

- **Domínio e política:** B-01 a B-05. Autoridade sobre significado das perguntas e conclusões.
- **Engine e execução:** B-06 a B-08. Pode trabalhar em paralelo após B-03, usando contratos e fixtures compartilhados.
- **Avaliação:** corpus e protocolo de B-09 começam junto de B-01; comparação usa o runner concluído.
- **Persistência e integração:** B-10 a B-14, depois da validação. Planejamento de schema/IPC pode antecipar; não acoplar o primeiro runner a essas entregas.

As frentes indicam responsabilidades, não agentes ou pessoas já designadas. Quando houver trabalho paralelo, cada tarefa deve declarar os arquivos que possui; mudanças nos tipos compartilhados exigem coordenação. Não iniciar implementações independentes de classificadores ou seletores.

## Verificação obrigatória

Para mudanças de implementação, executar os comandos de `AGENTS.md`:

```sh
# em app/
bun run lint && bun run typecheck && bun run test

# em app/src-tauri/
cargo test
```

O teste real de sessão exige o sidecar; um teste que pulou spawn não comprova integração real. Acrescentar testes dirigidos às novas invariantes, incluindo contexto histórico, evidência incompleta, conclusão versionada, tentativas finitas, eventos antigos, cache incompatível e teardown.

Benchmarks com engine falsa medem apenas a pipeline Rust. Resultados de Stockfish, IPC e renderização devem ser identificados separadamente.

## Próxima tarefa

Começar por **B-01**: fechar o contrato mínimo com exemplos e fixtures. Em seguida implementar **B-02 e B-03** como primeira mudança de código, mantendo B experimental. O primeiro objetivo verificável é M1; a primeira demonstração com Stockfish é B-07.

## Registro de andamento

| Data | Tarefa | Estado | Entrega / validação / pendências |
| --- | --- | --- | --- |
| 2026-11-03 | Plano | Criado | Backlog e dependências documentados; implementação ainda não iniciada |


## Registro de implementação parcial — 2026-11-03

O módulo isolado `review/decision` constrói decisões a partir do PGN validado, incluindo FEN raiz normalizada, prefixo UCI, limite do histórico e versão de política. A presença da tag FEN é obtida pelo parser, inclusive quando a FEN representa a posição inicial padrão. A evidência tipa cp/mate e bounds e exige que uma única execução irrestrita tenha profundidade comum por linha, identidade da execução, MultiPV efetivo limitado pelo número de lances legais, opções canônicas e digest hexadecimal de 64 caracteres idênticos, contexto idêntico e PVs legais com raízes distintas. O digest ainda é apenas um campo: não há cálculo de assinatura ou adaptador de engine. O score do filho muda de sinal para a perspectiva do jogador; +40 na raiz e -20 no filho significam +40 e +20 para o jogador da raiz.

A política parcial `b-experimental-1` reporta mate, afogamento, material insuficiente, 75 lances e cinco repetições conhecidas como obrigatórios. Cinquenta lances e três repetições já ocorridas são reivindicáveis e não encerram a análise; uma FEN inicial deixa explícito que o histórico anterior é desconhecido. Não se pretende inferir reivindicações por lance futuro nem repetições anteriores à raiz. Essa política ainda não está conectada a buscas e precisa de mais fixtures e revisão das regras antes de uso.

B-01 a B-03 estão **em andamento**, não concluídas: faltam manifesto, contratos completos de perguntas/conclusões e fixtures abrangentes. B-04 a B-07 permanecem **pendentes**: não existem planejador, engine fake de partida completa, driver, runner, relatório ou teardown para esta arquitetura. Portanto não houve execução de B com Stockfish e não se deve considerar o código uma entrega funcional. Persistência, UI e B-08+ não foram iniciados.

A inversão de perspectiva troca também bounds inferiores e superiores. A distância de mate continua referida à posição avaliada, sem ajuste entre raiz e filho nem projeção em centipawns; falta especificar essa conversão antes de classificar mates. A regra de reivindicação por lance anunciado ainda não está implementada. Os testes dirigidos atuais cobrem apenas parte dos cenários da matriz de QA.

Verificação do snapshot parcial: `cargo test review::decision --lib` passou 4 testes; `cargo fmt --check`, `cargo test` (140 passaram, 3 ignorados, handshake 1/1) e o teste específico de sessão real (1/1, sem skip) passaram. Em `app/`, lint e typecheck passaram, downloader 5/5 e Vitest 141/141. A sessão real testada é a pipeline existente, não B. Há avisos `dead_code` porque o módulo B ainda não está integrado. O runner B e seu processo não existem neste snapshot; nenhum JSON real ou teardown B foi verificado.

## Atualização de domínio — 2026-11-03

O planejador experimental agora executa cobertura contextual N+1, publica screening incremental, limita verificações após cobertura e registra conclusões, pendências e ledger serializáveis. `Planner` implementa o trait `runner::Plan`; a API e as limitações de regras estão em `analysis-b-contract.md`. `cargo test review::decision --lib` passou 9 testes no estado observado, incluindo replay fake, rejeição de eventos antigos, verificação finita e filho em xeque-mate sem busca. Esse comando inclui um teste do driver concorrente; não comprova runner real ou teardown.

B-01 a B-05 permanecem **em andamento**, sem aceite integral: faltam fixtures abrangentes de limites e histórico (inclusive um lance legal, promoção, roque e en passant), testes de entregas adversas além do worker único e política de descoberta tardia entre decisões vizinhas. A revisão atualmente verifica apenas decisões críticas no screening e não propaga evidência de verificação para abrir novas obrigações nas vizinhas. O ledger registra profundidade autorizada e parede observada, mas não mede trabalho de engine, CPU ou memória. O caminho de falha do trait do runner e a aceitação da evidência precisam ser integrados e verificados pelo responsável do runner. O manifesto é a estrutura `Config`, ainda sem fixture de arquivo ou validação de digest real no executor. Os estados acima substituem qualquer afirmação anterior de inexistência de planejador; não implicam que M1 ou B-07 estejam concluídos.

## Atualização de propagação — 2026-11-03

O planejador compartilha a evidência de verificação mais recente entre posições vizinhas, publica revisões com suas referências sem alterar o screening e reconsidera decisões já examinadas. Cada decisão tem orçamento finito próprio; cobertura inicial continua prioritária. O teste dirigido de descoberta tardia e conflito persistente passou, junto dos testes de contexto, promoção, perspectiva de mate e limites de repetição/50/75 lances. A integração real B-06/B-07, teardown, fixture de um lance legal e demais casos da matriz QA permanecem pendentes de verificação; este registro não declara entrega real concluída.

### Atualização — integração experimental na interface

A autorização posterior do usuário antecipou uma integração temporária: seleção Experimental B, sessão proprietária com lease compartilhado, handshake/digest, eventos próprios e resultado navegável sem cache/histórico. Testes de sessão com Stockfish real e suites frontend/Rust passaram. Isso não conclui B-01 a B-07 nem B-12/B-13: permanecem fixtures/adversos e validação visual integrada. B-08/B-09 e cache/histórico B-10/B-11 continuam pendentes; não há evidência comparativa de qualidade/custo ou decisão de adoção B-14. As listas antigas descrevem critérios, não ausência de código.

A revisão integrada identificou e corrigiu a seleção da FEN B para análise ao vivo e separou as candidatas do lance (posição anterior) da análise ao vivo da posição exibida. A revisão B permanece temporária.

### B-08 — trabalho parcial no workspace

O ledger agora identifica buscas de cobertura e de verificação crítica. O driver
mantém no máximo dois ciclos incompletos de profundidade, além da última evidência
completa; ciclos antigos podem ser descartados e não são conclusões. CPU e memória
continuam ausentes (`null`), não estimadas a partir do orçamento de busca.

B-08 permanece **em andamento**: `Planner` ainda possui uma única reserva ativa e
`runner::run` adquire um único processo. Portanto não existem despacho concorrente,
divisão de Threads/Hash, testes de conclusão fora de ordem ou drenagem de vários
processos B. O progresso integrado publica somente início e fim; não há coalescência
de progresso nem medição agregada de parede, CPU, memória, trabalho descartado ou
latência de cancelamento. A limitação dos ciclos do driver não limita, por si só,
as filas do transporte. M2 não está concluído.

### Atualização — concorrência e persistência experimental

O estado acima é histórico. A sessão B agora usa até dois workers com orçamento
total de 2 Threads e 32 MiB de Hash, aceita conclusões fora de ordem e drena os
processos em cancelamento, falha e panic. O cache contextual versionado está
conectado ao executor; o histórico guarda um snapshot independente e pode ser
reaberto sem engine mesmo após limpeza do cache. A gravação do histórico continua
best-effort. Testes de cache, falhas, drenagem e sessão com Stockfish real passaram.

Os marcos permanecem parciais: faltam observabilidade agregada e latência de
cancelamento (B-08), comparação reproduzível de qualidade e custo (B-09), cobertura
adversa adicional de sessão e validação visual integrada (B-12/B-13), e exercício
de migração e fallback com decisão de adoção baseada em dados (B-14). O protocolo
e a decisão provisória estão em `analysis-b-evaluation.md`; B continua experimental.

### Atualização — progresso, restauração e observabilidade

O executor publica snapshots coalescidos durante a execução, e a interface exibe
decisões preliminares sem reiniciar a navegação quando uma revisão chega. A
restauração valida a estrutura, a ordem e as posições do resultado salvo e rejeita
dados inválidos sem iniciar outra análise silenciosamente. Testes dirigidos cobrem
eventos obsoletos, migração e coexistência dos históricos.

Uma inspeção no navegador com dados controlados cobriu progresso e pendência e
encontrou uma falha na inicialização do tabuleiro somente de leitura, corrigida.
Essa inspeção não substitui uma validação visual integrada com Tauri e Stockfish.
O executor registra tempos, contagens e medições de processo quando suportadas;
detalhes e limitações estão em `analysis-b-runner.md`.
