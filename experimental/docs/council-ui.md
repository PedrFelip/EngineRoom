# Revisão da UI e ajustes dos councils

Toda a implementação permanece em `experimental/`; o app Tauri continua como
referência sem alterações. A interface de produção inicia sem avaliações ou
partidas inventadas. `fixtures.rs` abastece os registros reais de apresentação
somente em cenas headless e testes.

## Council Luna — revisão geral e tabuleiro

Modelo solicitado: GPT-6 Luna, esforço low, três revisões independentes.

| Lente | Rodada 1 | Rodada 2 |
| --- | --- | --- |
| Espaço e layout | 7/10 | 8/10 |
| Hierarquia e navegação | 7/10 | 8/10 |
| Responsividade, regressões e acessibilidade | 6/10 | 7/10 |

Os dois pareceres visuais da rodada 2 consideraram o escopo pronto. O terceiro
condicionou a entrega à conclusão do headless: ele consultou o log anterior,
interrompido por uma asserção que exigia foco em ações de limite desativadas.
O checker foi corrigido depois dessa revisão, sem reativar botões indisponíveis.
A execução final e seu relatório, não os artefatos antigos, são a evidência de
aprovação da matriz. Não houve uma nova nota do terceiro revisor após esse ajuste.

### Mudanças aplicadas

- Tamanho do tabuleiro limitado pela largura e pelo orçamento dos elementos
  renderizados, sem reserva fixa de 324 pt nem teto de 640 pt.
- Breakpoint derivado da largura do inspector, do tabuleiro e dos gutters.
- Em janelas estreitas, seletor persistente Tabuleiro/Lances/Análise/Resumo;
  ferramentas com scroll próprio e navegação de lances fixa, sem precisar
  descobrir o inspector abaixo da página.
- Em janelas largas, inspector lateral preservado, cabeçalho menor e controle
  Linhas junto às ferramentas. A preferência de visibilidade da dock é mantida.
- Navegação do tabuleiro em grupo compacto com Primer Octicons, contador,
  tooltips, nomes acessíveis e ações de limite desativadas.
- Momentos críticos apresentados por contador e atalhos anterior/próximo, sem
  repetir a lista inteira de lances críticos.
- Dica do toggle dos painéis mostra a ação atual e o atalho Ctrl+Alt+B. Esse
  atalho controla os painéis; não é anunciado como atalho do toggle Linhas.

## Correções dos councils anteriores

- Selecionar ou reabrir uma revisão não substitui o rascunho de importação.
- Carregamentos do histórico carregam IDs; navegação, seleção e fechamento
  invalidam pedidos anteriores antes de aplicar suas respostas.
- Fechar uma sessão a partir de Histórico/Configurações mantém a tarefa atual,
  sem abrir uma análise substituta ou redirecionar à revisão.
- Títulos completos e estado detalhado ficam na dica da sessão; o estado
  visível usa palavras curtas para evitar truncar a explicação principal.
- O botão de fechar fica em grupo sem seleção automática ao receber foco.
  Um teste percorre com Tab e exige Space para ativar; focar não pode fechar.
- Um teste explícito com Stockfish interrompe depois de progresso real,
  comprova cache parcial persistido, reabre e conclui usando cache. Outros
  testes reais cobrem live/cancel/fechamento/histórico e troca de sessão.
- README e comentários foram atualizados para remover promessas de controles
  de demo que já não existem na produção.

## Verificação

- `cargo fmt --all -- --check`
- `cargo test --workspace --locked`: 157 testes aprovados; quatro testes reais
  permanecem ignorados no comando padrão e exigem execução explícita.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo doc --workspace --no-deps`
- Os três testes `real_stockfish` da UI foram executados explicitamente com
  `ENGINE_ROOM_STOCKFISH`, Stockfish real e bancos temporários: aprovados.
- `scripts/check-responsive.sh`: aprovado nos quatro tamanhos documentados.
- `python3 scripts/test-headless-report.py`: dez testes aprovados, inclusive
  rejeição de inspector que ultrapasse a altura da janela.
- `scripts/check-headless.sh`: exit 0; 160 casos aprovados, zero falhas.
  PNGs e árvores foram regenerados. A revalidação `--analyze-only` também passou
  com os checks atuais, incluindo limites verticais do inspector.
  Relatórios: `artifacts/review-matrix/report.{md,json}`.

Capturas estáticas verificam apresentação e árvores; não substituem os testes
reais da engine. Não foi executada uma consulta ao serviço público Chess.com
para este ajuste visual: seus testes usam HTTP injetado.
