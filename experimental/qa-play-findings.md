> Registro histórico da revisão inicial. As correções e os resultados finais estão em [qa-multisession.md](qa-multisession.md).

# QA/review Rust: partidas Rune

Aplicação Rust edição 2024, sem MSRV declarado. Escopo: partidas, navegação entre sessões e análise; código compartilhado não commitado. O contrato atualizado exige que buscas continuem em segundo plano ao trocar de tela ou sessão, com respostas e cancelamentos vinculados à partida. Verificações no estado anterior à integração: `cargo fmt --check` passou; `cargo clippy -p engineroom-rune --all-targets --all-features --locked -- -W clippy::pedantic` terminou com avisos preexistentes; `cargo test -p engineroom-rune play --locked` passou 19 testes e ignorou dois que exigem Stockfish real. Esses testes existentes ainda validam o contrato antigo de cancelamento ao navegar e precisam ser atualizados. Nenhum teste novo foi registrado ou executado nesta revisão.

## Corrigir

### 1. Navegação cancela a busca e descarta a resposta da partida original (`crates/engineroom-rune/src/play_state.rs:147`, `:89`)

**Contrato:** a partida segue jogando em segundo plano; uma resposta pertence à sessão que iniciou a busca e fechar ou desistir afeta somente essa sessão. A versão examinada chama `stop_play_search()` em `park_play`, enquanto `receive_play_move` exige a página Play e usa apenas o jogo ativo. Ao trocar para outra partida ou análise, o motor é cancelado; mesmo uma resposta já em trânsito é descartada. A atualização em andamento pelo responsável introduz pedidos por sessão: verificar que a resposta atualiza a sessão original, que o cursor histórico permanece onde estava e que encerrar outra sessão não a cancela. O teste existente `navigation_cancels_search_and_retains_match` afirma o comportamento antigo e deve ser substituído, assim como a expectativa de resposta descartada no teste de 15 partidas. Reproduzido por inspeção do fluxo anterior; ainda não há teste novo executado para o novo contrato.

**Correção sugerida:** associar request e estado da busca à sessão; localizar a sessão pelo request na entrega, e cancelar apenas o request da sessão fechada ou desistente. Preservar a busca ao estacionar a partida.

### 2. Voltar pela navegação descarta promoção pendente (`crates/engineroom-rune/src/state.rs:867`, `:895`)

**Regra:** preservar o estado da sessão ao alternar contextos (invariante de estado, Rust best practices §1). `park_play` salva a promoção, mas `navigate(Page::Play)` reutiliza o jogo ativo sem restaurar esse campo e o limpa incondicionalmente. Após escolher origem/destino de promoção, abrir Home e voltar por Jogar, o diálogo desaparece. A jogada ainda não foi feita e precisa ser refeita. O teste `returning_to_play_from_home_preserves_pending_promotion` falharia na primeira comparação (`None` versus `Some((8, 0))`); inferência por leitura, não execução.

**Correção sugerida:** restaurar a promoção salva ao voltar à sessão, depois da limpeza de estado transitório, ou entrar pela mesma rotina de ativação que restaura o snapshot.

### 3. Fechar partida estacionada altera promoção da análise ativa (`crates/engineroom-rune/src/play_state.rs:191`)

**Regra:** operações de uma sessão não devem alterar o estado transitório de outra (invariante de estado, Rust best practices §1). `play.active` continua apontando para a última partida depois de abrir uma análise; fechar essa partida executa `selected_square = None` e `promotion_pending = None` mesmo na página de análise. Assim, uma promoção de variação em andamento é cancelada ao fechar uma partida inativa na tela. `closing_parked_play_does_not_clear_review_promotion` falharia na comparação após `close_play` (`None` versus `Some((8, 0))`); inferência por leitura, não execução.

**Correção sugerida:** limpar seleção e promoção globais somente quando a partida fechada está visível; estado da análise deve continuar intacto.

## Integração e cobertura

Solicitação ao responsável: registrar o novo arquivo como módulo filho de `state`, por exemplo `#[cfg(test)] #[path = "play_qa_tests.rs"] mod play_qa_tests;` em `state.rs`, e executar `cargo test -p engineroom-rune play_qa_tests --locked` a partir de `experimental/`. Não fiz o registro porque a alteração de outros arquivos está fora do escopo autorizado. Os testes adicionados cobrem promoção ao retornar de Home, fechamento de partida estacionada durante promoção na análise, 15 partidas com cursores históricos e fechamento ativo/inativo, e desistência no histórico seguida de análise do jogo completo. A posição de promoção é uma fixture sintética para atingir o estado sem dezenas de lances; os testes verificam ações e resultados visíveis.

Para o novo contrato de busca, acrescentar testes com bridge controlável: iniciar buscas em A e B, entregar respostas em ordem inversa enquanto uma análise está ativa, conferir lance apenas na sessão de origem; fechar/desistir A e verificar que B continua e que resposta tardia de A é ignorada; navegar no histórico enquanto chega lance e verificar que cursor não avança; repetir ao alternar 15 partidas/análises. Esses casos dependem da API de pedidos por sessão em implementação e não foram declarados como executados aqui.

## Pontos positivos

- O cursor histórico bloqueia lances humanos e a resposta do motor preserva o cursor quando o usuário não acompanha o último lance.
- PGN de desistência contém a lista completa de lances e o resultado, permitindo analisar a partida.
- O snapshot de partida já armazena promoção, orientação, cursor e erro ao alternar diretamente entre sessões.
