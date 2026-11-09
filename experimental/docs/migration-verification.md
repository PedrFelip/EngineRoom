# Verificação da migração para Rune

Baseline observada em 2026-11-08, na árvore de trabalho em andamento. Os testes abaixo não comprovam integração nativa com Stockfish, SQLite ou sessões: a interface experimental ainda contém cenários simulados. Nenhum banco do usuário foi aberto e nenhum download foi feito.

| Comando, a partir de `experimental/` | Resultado | Alcance |
| --- | --- | --- |
| `python3 scripts/test-headless-report.py` | exit 0; 9 testes | Testa o relatório headless, não a renderização nem o backend. |
| `cargo test --locked --offline` | exit 0; 18 testes | Testes da interface e lógica local, inclusive cenários mockados; não é teste de integração real. |
| `scripts/check-headless.sh` | não executado | Gera PNGs e árvores em `experimental/artifacts`, fora da área atribuída para esta verificação. |
| `scripts/check-responsive.sh` | não executado | Também grava em `experimental/artifacts`; responsividade visual não foi verificada nesta execução. |

Não houve falha nos comandos executados. A suíte do backend original e o teste de sessão com Stockfish real não foram executados; sua disponibilidade e o sidecar real não foram verificados. Os comandos acima são uma fotografia do WIP e devem ser repetidos após alterações concorrentes.

## Estado atual

O frontend Rune já usa o backend nativo em crates: `engineroom-domain`,
`engineroom-application`, `engineroom-uci`, `engineroom-sqlite` e
`engineroom-backend`. A análise abre o Stockfish nativo pelo adaptador UCI, e o
cache e o histórico usam um SQLite experimental próprio em
`engineroom-experimental/engineroom.db` dentro do diretório de dados do usuário
(no Linux, `$XDG_DATA_HOME`), separado do banco do aplicativo principal.

- Importação: colar PGN, abrir um arquivo por caminho e usar o seletor nativo de
  arquivos. Quando o seletor não está disponível, o caminho digitado continua
  sendo o fallback.
- Chess.com: busca por usuário com HTTP nativo, paginação ("Mais partidas") e
  limites de arquivos, candidatos, resposta, PGN e tempo.
- Histórico: lista paginada com "Carregar mais", reabertura, reanálise e exclusão
  de revisões salvas.
- Preferências: documento versionado em
  `engineroom-experimental/preferences.json` no diretório de dados do usuário
  (no Linux, `$XDG_DATA_HOME`; versão 1, escrita atômica e limites validados). A seção Análise envia linhas, tempo, threads e
  memória ao motor ao vivo.
- Armazenamento: estatísticas reais do SQLite experimental, limpeza de cache e,
  somente após confirmação explícita, limpeza do histórico.

Ainda não existem som/áudio conectado nem empacotamento ou release do Rune. As
cenas headless continuam determinísticas e não abrem Stockfish nem banco.

## Checklist de regressão para a integração

Os itens são critérios a verificar, não resultados já obtidos. Referências originais: `app/src-tauri/src/review/{engine,session,pipeline,repository}`, `app/src-tauri/src/db/{cache,migrations}.rs` e seus testes.

- [ ] Importar PGN legal, inclusive com FEN inicial, e rejeitar linha principal ilegal; metadados e configuração ao salvar/reabrir devem refletir o PGN e revisões antigas devem normalizar corretamente.
- [ ] Análise manual utiliza um processo; automática respeita perfis compartilhados, limites de trabalhadores, divisão de Threads/Hash, blocos de quatro posições, refinamentos e quotas. Resultados aparecem e são cacheados na ordem de conclusão; linhas preliminares sobrevivem ao refinamento.
- [ ] Fragmentar saída UCI entre leituras, inclusive linhas parciais; somente mensagens estruturadas chegam à interface. Falha ou interrupção de busca ao vivo encerra e aguarda o processo antes de reutilizar recursos.
- [ ] Disparar navegações rapidamente e cancelar uma busca em curso: apenas o pedido mais recente produz resultado visível, depois da limpeza anterior; eventos de sessão, sequência ou pedido antigos são descartados.
- [ ] Concorrer sessão e probe de configurações: a concessão exclusiva só passa ao próximo proprietário após todos os processos anteriores terminarem; fechar duas vezes é seguro, destruir janela cancela e sair aguarda limpeza e operações de banco pendentes.
- [ ] Para cache de profundidade, aceitar profundidade alcançada suficiente mesmo se originada por tempo; para cache de tempo, exigir entrada de tempo com orçamento suficiente. Ambos exigem MultiPV suficiente; nunca comparar numericamente tempo com profundidade.
- [ ] Prefetch em lote preserva correspondência com FENs; escrita ocorre a cada oito posições concluídas e ao final, com transação atômica para coberturas mistas. JSON corrompido resulta em miss; erro real de SQLite falha a análise e flush de falha não substitui o erro original.
- [ ] Conclusão da revisão aparece antes da gravação de histórico; falha de gravação não invalida a revisão, mas encerramento aguarda a operação. Reabrir, reanalisar e excluir com banco temporário e verificar migrações repetidas de esquemas antigos.
- [ ] Executar teste real com Stockfish empacotado e banco temporário: analisar, reutilizar cache, salvar/reabrir histórico, substituir e cancelar busca ao vivo e verificar ausência de processos órfãos. Testes com engine falsa não substituem essa prova.
- [ ] Repetir testes headless e responsivos em tamanhos mínimo e amplo, temas claro/escuro, navegação, painéis e variações, conferindo árvores e imagens; incluir fluxos reais quando conectados ao backend.

## Verificações posteriores da integração

Na mesma data, o root executou `scripts/check-headless.sh` completo com exit 0: a matriz contém 160 casos e zero falhas, além das verificações de cenas, árvores e sidebars. `scripts/check-responsive.sh` também terminou com exit 0. Essas capturas precedem a conclusão da integração live e continuam usando fixtures determinísticas; não provam o fluxo real de engine na UI.

O backend nativo recebeu testes de persistência, sessões, pipeline, pool, importação HTTP injetada e transporte com backpressure. Após a correção de envio assíncrono, o agente da engine reportou 107 testes aprovados e um teste real ignorado no comando padrão; a execução explícita de `real_native_stockfish` com o sidecar passou. Clippy com todos os targets/features e documentação da crate passaram nessa revisão. Nenhum teste abriu o banco do app principal. Esses resultados precisam ser reconfirmados após a integração final da UI.

## Verificação consolidada da integração

Executada em 2026-11-08, após a consolidação do WIP dos agentes, na árvore de
trabalho (sem commit).

| Comando | Resultado |
| --- | --- |
| `cargo fmt --check` em `experimental/` | exit 0. |
| `cargo clippy --all-targets --all-features -- -D warnings` em `experimental/` | exit 0, sem avisos. |
| `cargo test --workspace` em `experimental/` | 150 aprovados, 0 falhas, 2 ignorados; inclui as crates. |
| `./scripts/check-headless.sh` | exit 0; 160 casos da matriz, 0 falhas, mais cenas, árvores e sidebars. |
| `./scripts/check-responsive.sh` | exit 0 em 720×560, 900×700, 1024×768 e 1440×1080. |
| `bun run lint && bun run typecheck && bun run test` em `app/` | exit 0; 130 testes. |
| `cargo test` em `app/src-tauri/` | 115 aprovados, 0 falhas, 3 ignorados. |
| `cargo test real_stockfish_bridge_completion_live_cancel_and_reopen -- --ignored` com `ENGINE_ROOM_STOCKFISH=app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu` | exit 0; análise, cache, reabertura e cancelamento ao vivo. |
| `cargo test -p engineroom-uci --test native_engine real_native_stockfish -- --ignored` com o mesmo sidecar | exit 0. |

Os scripts headless continuam usando fixtures determinísticas: eles verificam
layout, árvores de acessibilidade, navegação e interações, não o fluxo visual com
Stockfish ou SQLite reais. O fluxo nativo é coberto pelos testes de bridge e de
transporte acima, incluindo banco temporário e o sidecar empacotado; nenhum teste
abriu o banco do aplicativo principal. A inspeção visual manual das capturas e o
empacotamento/release do Rune continuam pendentes.

## Validação final da UI após councils

A árvore atual passou em formatação, 157 testes do workspace, Clippy com todos
os targets/features e warnings como erros, e documentação. Há quatro testes
reais ignorados no comando padrão. Os três testes reais da UI foram executados
explicitamente com Stockfish e bancos temporários e passaram, incluindo retomada
de análise interrompida com cache parcial.

`check-responsive.sh` terminou com exit 0. `check-headless.sh` terminou com exit 0:
160 casos, zero falhas. Os dez testes do coletor e a revalidação das árvores
com `--analyze-only` passaram. Os verificadores respeitam ações desativadas nas
extremidades e rejeitam inspectors que ultrapassem a altura da janela.

Veja `council-ui.md` para achados e correções. Os resultados históricos acima
são baselines; este registro descreve a implementação posterior às correções.
