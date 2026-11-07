# QA independente — pipeline B, B-01 a B-07

Estado observado em 2026-11-03, durante implementação concorrente; não é aceite final. O código pode mudar depois desta inspeção. O escopo é o runner experimental, sem UI ou SQLite.

## Matriz de aceitação e rechecagem

| Marco | Cenários e resultado exigido | Evidência observada / pendente |
| --- | --- | --- |
| B-01 | Documento versionado especifica evidência para melhor/jogado, gatilhos e orçamento crítico, tentativas, unresolved, estados e política de repetição, 50/75 lances e histórico anterior à raiz; assinatura inclui digest real e opções. Fixtures: tática, mate, promoção, repetição, FEN e partida longa. | Plano ainda marca pendente; não identificar a string de política com uma especificação completa. Validar política e fixtures após DEV. |
| B-02 | PGN de N lances produz N decisões, com raiz e prefixos exatos; FEN inicial com histórico anterior desconhecido; mesma FEN obtida por históricos distintos não é intercambiável. Testar repetição conhecida/raiz desconhecida, 99/100/149/150 halfmoves, jogada que zera contador, xeque-mate no limite, roque e en passant. Resultado terminal segue política explícita, sem busca desnecessária. | Inspeção inicial: `decisions` usa `core::extract`, raiz, prefixo e `BeforeRootUnknown` para tag FEN; testes mínimos de prefixo e promoção. Política terminal contextual e limites não verificados. |
| B-03 | Testar sinais de centipawns e mate para ambos os lados, incluindo mate zero; bounds inferior/superior nunca viram exact. Uma única evidência deve satisfazer simultaneamente contexto, digest/opções, escopo, profundidade, MultiPV e linhas legais/distintas; profundidade de uma execução e MultiPV de outra não se somam. Verificar revisões versionadas, screening imutável e origem terminal sem execução inventada. | `satisfies` observado exige uma evidência individual, contexto/assinatura iguais, busca irrestrita, profundidade, linhas exact, PV legal e raízes distintas; teste cobre bound, raiz duplicada e PV ilegal. Perspectiva/mate, compatibilidade adversa e revisões ainda exigem testes. |
| B-04 | Replay determinístico de partida inteira com entregas em ordens adversas: dependências vizinhas compartilham evidência compatível, não há pedido ativo duplicado, publicação preliminar ocorre antes do fim; eventos de geração/execução antiga não mudam estado. | Planejador e replay completo não observados na inspeção inicial; verificar após implementação. |
| B-05 | Conflito persistente esgota tentativas e termina unresolved com motivo; expansão e verificações antecipadas limitadas, cobertura progride com um worker. Descoberta posterior referencia origem e gera nova conclusão sem alterar screening. Ledger reconcilia autorização, reserva e gasto por execução; medições ausentes serializam null, não zero. | Não observado na inspeção inicial; testar também falha técnica e cancelamento distintos de unresolved. |
| B-06 | Fake registra `position` com raiz e prefixo, opções fixas, um leitor e um go ativo. Intercalar slots e profundidades, bounds, PV ilegal/duplicada, mate e atualização parcial: apenas conjunto completo coerente à mesma profundidade é aceito; parcial posterior conserva último completo; sem conjunto válido resulta ausência explícita. Testar saída inesperada, timeout e cancelamento, aguardando morte do processo. Tempo autorizado não é tempo efetivo. | Driver contextual não observado na inspeção inicial. Testar Stockfish real e inspecionar rastreamento da posição, não inferir contexto apenas do resultado. |
| B-07 | Runner de um worker consome PGN e manifesto reproduzíveis; exporta versão, assinatura, evidências, conclusões, pendências e ledger auditável, sem abrir/escrever DB de produção. Em sucesso, falha e cancelamento, verificar processo encerrado antes do retorno. Dentro do app, lease existente permanece até cleanup. | Runner não observado na inspeção inicial; comando real e artefato ainda necessários. Skip não comprova integração. |

## Verificações e comandos

- Em `app/`: `bun run lint && bun run typecheck && bun run test` — concluído nesta inspeção: lint e typecheck passaram; downloader 5/5 e Vitest 141/141 passaram. Não demonstra cobertura integral de B.
- Em `app/src-tauri/`: `cargo test` e `cargo test real_stockfish_sidecar_cache_history_reopen_and_live_cancellation` — `cargo test` passou: 139 testes, 3 ignorados; handshake real passou. O teste de sessão real executado separadamente passou: 1 teste, sem skip. A compilação ocorreu durante alterações concorrentes; repetir no estado final.
- Sidecar presente na inspeção: `app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu`. Sua presença não comprova execução real.
- Após DEV: executar novamente os comandos acima sobre o estado final; executar também comando documentado do runner real com PGN e manifesto fixos, guardar saída e código de saída, conferir identidade/digest e contagem de decisões, e testar encerramento de processos em sucesso, erro e cancelamento. Comando exato depende da interface ainda em implementação.

## Riscos prioritários para revisão

1. Histórico e regras de empate: `core::terminal_position` legado considera 100 halfmoves terminal; B precisa aplicar a política versionada, inclusive distinção entre empate reivindicável e automático e histórico desconhecido.
2. Cobertura falsa: misturar profundidades, slots, bounds ou execuções pode produzir uma conclusão aparentemente completa sem evidência suficiente.
3. Cleanup e eventos: confirmar processo efetivamente aguardado e rejeição de eventos antigos antes de modificar conclusões, inclusive em erro e cancelamento.
4. Identidade e auditabilidade: digest efetivo, opções, contexto e ausência de medições precisam sobreviver ao relatório; não presumir qualidade estatística de gatilhos.

A implementação mudou durante a verificação: a compilação posterior já mencionava `PositionStatus` e `position_status`, ausentes na primeira leitura. Os resultados acima não constituem revisão desse código novo nem aceite do estado final.

## Rechecagem integrada independente — 2026-11-03

Snapshot de trabalho com alterações preexistentes, sem commit. Aceite abaixo é parcial: não se infere conclusão global a partir de uma partida curta.

| Marco | Aceite | Evidência e lacunas |
| --- | --- | --- |
| B-01 | Parcial | Contrato versionado descreve scores, cobertura, limites e empates; faltam exemplos e corpus/fixtures completos do plano. O manifesto externo ainda não existe. |
| B-02 | Parcial | Testes de domínio cobrem prefixos, FEN, promoção, repetição e limites 99/100/149/150; falta matriz de roque, en passant, reinício do contador e mate no limite. |
| B-03 | Parcial | Testes exercitam cobertura individual, profundidade, execução, bounds, PV ilegal/duplicada e inversão; screening e revisões são testados no planner. Matriz adversa completa de identidade e perspectiva permanece pendente. |
| B-04 | Parcial | Replay e eventos obsoletos são testados; cobertura inicial e compartilhamento existem. Não há demonstração de entregas fora de ordem com múltiplos workers; o contrato atual tem uma execução ativa. |
| B-05 | Parcial | Teste de descoberta tardia e conflito persistente exercita limite e propagação; ledger real informa parede e CPU/memória null. Faltam outros cenários adversos e reconciliação abrangente. |
| B-06 | Parcial | Driver fake cobre conjuntos por profundidade, bounds e parcial posterior; Stockfish real confirmou um lance legal com MultiPV 2. Cleanup real observado em sucesso, timeout e cancelamento; timeout/cancelamento observados durante handshake, não durante busca. Matriz de protocolo e ownership automatizado específica de B faltam. |
| B-07 | Parcial | Runner real exportou JSON versionado com decisões, evidências, revisões, pendências e ledger; exemplo não usa SQLite. Não aceita manifesto externo e não está integrado ao lease do app. Cleanup observado, mas não automatizado; não há aceite integral. |

Verificação no snapshot: `bun run lint && bun run typecheck && bun run test` passou (downloader 5/5, Vitest 141/141); `cargo fmt --check && cargo test` passou (148 testes, 3 ignorados, handshake 1/1); `cargo test real_stockfish_sidecar_cache_history_reopen_and_live_cancellation` passou 1/1 sem skip. O teste de sessão e o teste de lease da pool pertencem ao caminho existente, não demonstram ownership de B.

Reprodução real, a partir de `app/src-tauri/`: criar `/tmp/b-qa-mate.pgn` com `1. f3 e5 2. g4 Qh4# 0-1`, executar `cargo run --example analysis_b -- /tmp/b-qa-mate.pgn binaries/stockfish-x86_64-unknown-linux-gnu /tmp/b-qa-direct.json`. Saída: status complete, 4 decisões, 7 evidências, 4 screenings, 5 revisões, 2 unresolved, 7 entradas de ledger. SHA-256 declarado `0f83d24cc46d2c66c60f16001af5444873bc112b7d028594513426894c12da19` corresponde ao arquivo do sidecar neste host; opções Hash 16, MultiPV 2, Threads 1. CPU e memória são null. `complete` não significa ausência de pendências.

Fixture real de um lance legal: `[SetUp "1"]`, `[FEN "5Nkr/1N5b/2P5/5P1p/p3p3/1P6/5qBp/4bKQ1 w - - 5 68"]`, seguido de `68. Qxf2 *`. Com o mesmo comando e PGN substituído, `/tmp/b-qa-one.json` registrou uma decisão e duas evidências: raiz em profundidade 8 com uma linha `g1f2`, filho em profundidade 8 com duas linhas, status complete. É uma execução real, não uma fixture automatizada permanente.

Para observar cleanup, `/tmp/b-qa-engine` registrou o próprio PID e executou o sidecar com `exec`. O runner foi chamado com esse wrapper para sucesso, timeout de 1 ms e cancelamento após 1 ms; após cada retorno `/proc/<pid>` não existia. Timeout e cancelamento retornaram código 1 e JSON failed; ocorreram no handshake. Nesta instrumentação o digest identifica o wrapper, não o Stockfish executado por ele; a verificação de digest do sidecar foi feita separadamente com execução direta. Não se comprovou aqui cleanup em falha durante busca nem exclusividade de lease de B.

Bloqueios reproduzíveis para aceite integral: (1) CLI documentada aceita apenas PGN, executável, saída, timeout e cancelamento; não há argumento de manifesto configurável (`cargo run --example analysis_b --` mostra usage). (2) `runner::process::Factory` declara explicitamente não adquirir lease do aplicativo; B não está integrado à sessão. (3) a matriz acima e testes automatizados de ownership/teardown e do caso de um lance legal permanecem incompletos. Nenhuma avaliação de qualidade ou performance foi realizada.

## Correções após auditoria com skills Rust

A auditoria retrospectiva aplicou `rust-best-practices` e `rust-review`. A inversão de scores agora é verificada, e parser/evidência rejeitam `i32::MIN` (cp e mate), com regressões. A CLI cria a saída exclusivamente, recusando arquivos existentes para não truncar entradas, com teste. O contrato antigo do trait foi removido; o chamador precisa sinalizar cancelamento e aguardar o runner, sem abortar sua future.

Validação posterior: 13 testes de domínio, suite Rust 149 aprovados/3 ignorados (sessão real incluída), frontend lint/typecheck/downloader 5/Vitest 141 passaram. Clippy estrito continua bloqueado por `if_same_then_else` em automatic.rs e `too_many_arguments` em repository/operation_tests.rs, fora destas alterações B. A integração futura no app deve possuir o runner em task cujo encerramento seja aguardado.

## QA integrado frontend e sessão — 2026-11-03

Snapshot de trabalho sem commit. Esta seção substitui a afirmação anterior de que B não está integrado à sessão: o backend atual integra B em `run_session`. A verificação não incluiu interface visual nem execução de `tauri dev`.

- Seleção: por inspeção, `HomePage` envia `pipeline: experimental-b` em `onStart`; `ReviewScreen` escolhe `BReviewScreen`; `TauriBackend` repassa a configuração em `review_session_open`. A ausência de pipeline usa legacy, coberto pela validação Rust. A passagem inteira do clique até IPC não foi automatizada.
- Eventos: Schema testa progresso e conclusão B; teste do adaptador rejeita sessão diferente e sequências repetidas/antigas. `review-session.ts` encaminha progresso ao callback e resultado ao store; a view consome esses dados. São verificações em partes e inspeção, não um teste ponta a ponta da UI. O progresso implementado informa apenas início e fim da cobertura, não progresso intermediário ou verificação.
- Ciclo de vida: testes existentes cobrem descarte após aquisição, request antigo, cancelamento e close. O teste B com fake confirma que close espera a limpeza; o teste B real confirma conclusão e liberação do lease. Não demonstram cancelamento B durante busca real nem renderização visual.
- Persistência: por inspeção, o ramo B publica `bCompleted` e não entra no ramo legacy que chama `pipeline.save`; o resultado declara `temporary`. O teste real usa SQLite em memória, mas não verifica contagem de registros/cache. Compatibilidade legacy é coberta pela suíte e pelo teste real de cache, histórico, reabertura e cancelamento.

Comandos executados neste snapshot:

- `cd app && bun run lint && bun run typecheck && bun run test && bun run build`: sucesso; downloader 5/5, Vitest 143/143; build concluiu com aviso de chunk acima de 500 kB.
- `cd app/src-tauri && cargo test`: sucesso, 152 testes aprovados, 3 benchmarks ignorados, handshake real 1/1.
- `cargo test experimental_b_session_uses_real_sidecar_and_releases_lease`, `cargo test experimental_b_cancellation_waits_for_fake_process_cleanup` e `cargo test real_stockfish_sidecar_cache_history_reopen_and_live_cancellation`: cada teste passou individualmente, sem skip.

Não foi identificado bloqueio concreto para iniciar `bun run tauri dev` neste ambiente por inspeção e build; o comando não foi executado, portanto inicialização da janela e experiência visual continuam sem confirmação. Sidecar real está presente e foi executado pelo teste de sessão. Nenhuma conclusão sobre qualidade da análise ou desempenho é inferida desses testes.
