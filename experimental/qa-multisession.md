# QA e review Rust: V3 multicessão

Rodada de 16/11/2026, limitada ao workspace `experimental/`, Rust edição 2024.
Aplicação nativa, com portas injetáveis de motor e repositório. Três revisores
GPT-6.1 Terra com esforço low fizeram QA de partidas, recursos e navegação;
um quarto agente Terra low implementou o isolamento das revisões. O responsável
integrou as correções, ampliou as regressões e executou a validação final.
Os relatórios individuais descrevem a versão anterior às correções. Este documento
registra o estado final.

## Contrato de produto

- Trocar a tela não fecha nem cancela uma tarefa de partida ou análise.
- O lance do motor atualiza a partida que o solicitou, inclusive fora da tela.
- Consultar o histórico não muda a posição em que o usuário está navegando quando
  chega um lance novo. Para jogar, é preciso voltar à posição atual.
- Fechar ou desistir afeta somente a sessão correspondente. Respostas atrasadas e
  duplicadas não alteram outra sessão nem recriam uma partida fechada.
- Análises independentes têm progresso, resultados, erros, sequência e pedidos
  live próprios. Retornar a uma revisão não cria outra tarefa nem reinicia a análise.
- Até dois grupos de motor trabalham simultaneamente; em uma máquina com uma CPU,
  há um grupo. Pedidos excedentes aguardam capacidade, sem bloquear o frontend.
  Cada grupo possui seus processos e só libera capacidade após confirmar o teardown.
- Sair do aplicativo cancela e drena todas as tarefas. Sessões abertas ainda não são
  recuperadas após reiniciar o processo; revisões salvas continuam no histórico.

## Falhas encontradas e corrigidas

| Prioridade | Problema anterior | Correção e regressão |
| --- | --- | --- |
| Alta | Sair da partida cancelava o motor e descartava a resposta. | Pedido vinculado à partida; entrega fora da tela e em ordem inversa em 15 partidas, sem alterar a análise visível. |
| Alta | A revisão global de cancelamento invalidava pedidos de outras partidas. | Cancelamento e tarefa por pedido na Bridge; fake controlado prova que cancelar A não cancela B. |
| Alta | Abrir outra revisão encerrava a anterior e filtrava seus eventos. | Registro e filtragem por ID; snapshots recebem progresso, resultado e erros em segundo plano. |
| Alta | Revisão live concluída retinha o motor ocioso e bloqueava a fila. | `session/task.rs` descarta o processo após o job; teste aguarda a confirmação de teardown antes de outro dono adquirir o slot. |
| Alta | PGN de uma partida curta podia herdar o cursor da revisão anterior. | Preparar uma revisão inicializa seu próprio cursor e variação, sem passar pela navegação da partida. |
| Média | Promoção pendente sumia ao navegar ou era apagada ao fechar outra sessão. | Promoção e seleção restauradas por sessão; operações invisíveis não limpam a interface ativa. |
| Média | Importação tardia de arquivo substituía tela e rascunho novos. | Entrega condicionada ao contexto original de navegação, origem e rascunho. |
| Média | Abrir uma partida não invalidava uma abertura de histórico pendente. | Ativação invalida o pedido anterior; resposta tardia não muda a tela. |
| Média | Fechar uma partida estacionada durante criação ativava outro tabuleiro. | Só o fechamento da partida visível escolhe uma sessão substituta. |
| Média | ID de revisão duplicado podia silenciar a tarefa original. | ID duplicado é recusado sem modificar o registro existente. |

Regras utilizadas: Rust best practices §1 (invariantes e estado), §3 (ownership e
limites de recursos), §7 (concorrência e escopos de locks), §11 (regressões de
comportamento); contratos de sessão, navegação e teardown de `AGENTS.md`.

Pontos principais da implementação:

- [play_state.rs](crates/engineroom-rune/src/play_state.rs): registro dos pedidos,
  entrega à sessão original e preservação do cursor.
- [workspace.rs](crates/engineroom-rune/src/workspace.rs): snapshots e roteamento
  de eventos das revisões em segundo plano.
- [bridge.rs](crates/engineroom-rune/src/bridge.rs): tarefas e cancelamento por
  pedido, registro das revisões e controle dos limites live.
- [task.rs](crates/engineroom-application/src/review/session/task.rs): liberação
  de capacidade quando a sessão está ociosa.
- [state.rs](crates/engineroom-rune/src/state.rs): entrega de importações e
  inicialização independente das revisões.

## Evidência de testes

| Caso | Evidência |
| --- | --- |
| 15 partidas com lances fora da tela, entregues em ordem inversa | `fifteen_background_moves_route_to_their_sessions_without_touching_review_state` |
| Resposta duplicada, partida fechada ou desistência | Testes em `state::play_qa_tests` |
| Erro e retry locais à partida | `background_engine_failure_is_local_and_retry_gets_a_new_request` |
| Cancelar uma busca enquanto outra aguarda | `cancel_one_queued_or_running_match_does_not_cancel_another` |
| Duas análises realmente buscando ao mesmo tempo, com fechamento independente | `two_live_reviews_search_concurrently_and_closing_one_keeps_the_other_running`, portas controladas por canais e Notify |
| Três análises e uma partida usando Stockfish 19 real | `real_stockfish_multiple_reviews_and_background_match_complete_together` |
| Progresso fora da tela e rejeição de sequência antiga | `review_concurrency_tests` |
| Sair do aplicativo aguarda encerramento do processo | `drop_cancels_search_and_waits_for_engine_termination` |
| Importação tardia, configurações, histórico e rascunho | `navigation_qa_tests` |
| Promoção nas partidas e revisões, retorno do histórico, 15 sessões | `play_qa_tests` e `review_concurrency_tests` |
| Layout pequeno e amplo, lista de lances e dock | `check-play.py`, `check-responsive.sh` e matriz headless da revisão |

Validação final:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
ENGINE_ROOM_STOCKFISH="$PWD/resources/stockfish" cargo test --workspace --locked real_ -- --ignored --test-threads=2
./scripts/check-headless.sh
./scripts/check-responsive.sh
cargo doc --workspace --no-deps --locked
```

A suíte padrão executa 241 testes com sucesso. Sete testes com o Stockfish 19
real passaram na execução opt-in, incluindo análise, Elo,
cancelamento, reabertura, cache e concorrência. Dois cenários headless opt-in
também passaram nessa execução. Capturas geradas permanecem em
`artifacts/`, ignoradas pelo Git.

## Limites da verificação

Esta rodada foi executada em Linux. Não certifica comportamento de janela,
empacotamento ou processos em Windows/macOS. O teste de concorrência prova
sobreposição e isolamento, não um ganho de velocidade; não foi feito benchmark
end-to-end. O escalonamento é limitado e usa fila, sem preempção de uma revisão
longa em andamento. Tarefas abertas não têm persistência de workspace entre
reinicializações. Timeout e falha de transporte têm tratamento no código e
cobertura existente parcial; esta rodada não simulou todas as possíveis falhas
do sistema operacional nem todos os tempos de timeout reais.

## O que preservar

- Portas injetáveis de motor e repositório, que permitem controlar aquisição,
  resposta e término sem depender de sleeps para ordenar os testes.
- Validação de lances e PGN antes da revisão e rejeição de eventos obsoletos.
- Teardown aguardado antes de liberar capacidade e cancelamento por dono.
- Componentes compartilhados entre partida e revisão, testados em janelas mínimas.
