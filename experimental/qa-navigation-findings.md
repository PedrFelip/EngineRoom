> Registro histórico da revisão inicial. As correções e os resultados finais estão em [qa-multisession.md](qa-multisession.md).

# QA de navegação do Rune

Escopo: aplicação Rust experimental, edição 2024; histórico, importação, configurações, revisão e transições com partidas. A análise considera o workspace em alteração. O contrato atualizado exige que partidas continuem pensando fora da tela, com resposta e cancelamento associados à sessão de origem.

## Corrigir: leitura tardia de PGN muda a tela e apaga rascunho novo

- Local: `crates/engineroom-rune/src/state.rs:611` (consumo da leitura), `state.rs:867` (navegação), `state.rs:1211` (início da leitura), `state.rs:1230` (preparação e mudança de tela).
- Contrato: uma resposta assíncrona antiga não deve substituir uma intenção posterior de navegação nem texto mais recente. Relaciona-se à orientação de correção e concorrência de `rust-best-practices`, seções 4 e 7: representar invariantes e manter estados relacionados consistentes.
- Reprodução: iniciar leitura de arquivo PGN, navegar para Configurações, substituir o rascunho e então processar a resposta. O teste `delayed_file_read_does_not_override_navigation_or_new_draft` usa a bridge real e controla quando a UI processa a resposta. Esperado `(Settings, "1. d4 d5 *")`; obtido `(Configure, "1. e4 e5 *")`. O resultado indevido perde o rascunho mais recente e tira o usuário da tela atual.
- Causa: a resposta de `ReadFile` não carrega identidade da solicitação nem versão do rascunho; `poll_bridge` aplica o conteúdo e chama `prepare` incondicionalmente. A resposta do seletor também não é vinculada à intenção que a originou, mas essa variante não foi reproduzida separadamente.
- Correção sugerida: associar um identificador/geração ao seletor e à leitura, invalidá-lo ao abandonar ou substituir a intenção de importação e validar a versão do rascunho antes de aplicar. Rejeitar respostas vencidas sem afetar a tela nem o texto atual. Manter a conclusão de limpeza da operação independente da aceitação do resultado.

## Cobertura e limites

Três testes headless passaram: retorno de Configurações à revisão e preservação do rascunho; fechamento da revisão enquanto Configurações está aberta; entrada no filtro do histórico sem acionar atalhos da revisão. O teste da falha está marcado `#[ignore]` para não quebrar a suíte normal e deve ser executado explicitamente até a correção.

O código observado antes da implementação em andamento cancelava a busca ao estacionar a partida (`play_state.rs`, `park_play`/`stop_play_search`) e rejeitava respostas fora de `Page::Play` (`receive_play_move`). Isso contrariava o contrato atualizado; não classificamos como falha confirmada da implementação nova nem verificamos ainda cancelamento seletivo ou entrega entre sessões após essa mudança. O teste antigo `navigation_cancels_search_and_retains_match` expressava o contrato anterior e precisa ser substituído pelo novo contrato. A concorrência de exclusão e os diálogos de arquivo tardios não foram reproduzidos integralmente.

Pontos positivos: carregamento individual do histórico já usa identificadores para rejeitar respostas vencidas; a revisão preserva snapshots por sessão; o filtro do histórico não dispara atalhos de revisão no cenário testado.

## Verificação

Na cópia temporária com o módulo registrado em `main.rs`, `cargo test -p engineroom-rune navigation_qa_tests` passou (3 testes, 1 ignorado). `cargo test -p engineroom-rune navigation_qa_tests -- --include-ignored` reproduziu a falha descrita (3 passaram, 1 falhou). No workspace compartilhado: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo test --workspace --locked` passaram; o teste de Stockfish real da suíte estava ignorado por exigir `ENGINE_ROOM_STOCKFISH`. Os resultados refletem a versão em alteração no momento da execução.
