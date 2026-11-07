# EngineRoom / Rune

Frontend desktop experimental em Rust com [Rune](https://github.com/anomalyco/rune),
iniciado em [`examples/hello`](https://github.com/anomalyco/rune/tree/d03349e25189ce1edc8c26ada6bf6d4aac72c826/examples/hello).
Segue os fluxos do app Tauri: importar → configurar → revisar, histórico e
configurações com Preferências, Análise e Armazenamento.

```sh
cd experimental
cargo run --locked
```

Requer Rust estável (edição 2024), dependências nativas de Linux/X11/Wayland e
Vulkan disponível, inclusive Mesa lavapipe para renderização headless.

## Componentes e estrutura

Usa componentes reais de `rune-kit`, com ícones, movimento e paleta de
`rune-theme-opencode`. A tipografia usa **IBM Plex Sans** na interface e
**IBM Plex Mono** no PGN e nos dados numéricos, com fontes embutidas. As três dependências Rune estão fixadas no mesmo commit
`d03349e25189ce1edc8c26ada6bf6d4aac72c826`.
A interface usa a paleta original do OpenCode nos temas claro e escuro,
diretamente de `rune-theme-opencode`, incluindo superfícies, estados e elevações.

- `src/main.rs`: composição das telas, cenas headless e atalhos.
- `src/state.rs`: navegação tipada, rascunho PGN e preferências da sessão.
- `src/theme.rs`: paleta OpenCode (claro/escuro), escala tipográfica e fontes IBM Plex.
- `src/layout.rs`: medidas responsivas baseadas na área útil real do shell.
- `src/game.rs`: validação local de PGN/FEN e posições para a prévia.
- `src/components/ui.rs`: grupos, linhas de preferência, tipografia e slider acessível.
- `src/components/shell.rs`: `SplitView`, `Sidebar`, `SidebarRow` e `Toolbar` do kit.
- `src/components/home.rs`: `TextArea`, perfis segmentados e importação/configuração.
- `src/components/board.rs`: tabuleiro e identificação dos jogadores.
- `src/components/review.rs`: layout, playback, momentos críticos e controles de demonstração.
- `src/components/review_{analysis,feedback,graph,summary}.rs`: painéis e gráficos da revisão.
- `src/review_data.rs`: avaliações e classificações fictícias, candidatos legais determinísticos.
- `src/variation.rs`: exploração de lances legais sem alterar a linha principal.
- `src/components/history.rs`: partida disponível na sessão.
- `src/components/settings.rs`: `Switch`, `SegmentedControl`, `Select` e preferências.

Referências visuais: [Settings](https://github.com/anomalyco/rune/blob/dev/demo/settings-dark.png)
e [OpenCode](https://github.com/anomalyco/rune/tree/dev/demo/opencode).
A documentação de [rune-kit](https://github.com/anomalyco/rune/tree/dev/crates/rune-kit)
orienta a composição; a [documentação de Rune](https://github.com/anomalyco/rune/blob/dev/crates/rune/README.md)
orienta layout, escopos de eventos, acessibilidade e testes headless.

## O que funciona

- Colar e editar um PGN; validar lances legais, FEN de início e uma partida por importação.
- Configurar um perfil na interface e abrir a prévia, preservando o rascunho ao voltar.
- Navegar por clique, setas, Home/End e Shift+setas entre momentos críticos.
- Alternar Lances, Análise e Resumo; filtrar erros por cor; clicar no gráfico para navegar.
- Visualizar barra de avaliação, selos no tabuleiro/lista, setas e resumo por fase no mock.
- Selecionar candidatos legais, reproduzir uma linha, pausar e retornar à partida.
- Criar variações por clique ou arraste de peças (promoção automática para dama).
- Configurar linhas, feedback, tempo, threads e memória no modal da revisão.
- Simular progresso, cancelamento e erro com nova tentativa pelo controle de cenário.
- Alternar foco no tabuleiro e painéis, inclusive com a sidebar fechada.
- Trocar tema em toda a interface; mudar switches, sliders e linhas candidatas.
- Recolher e redimensionar a sidebar; `Ctrl+Shift+B` alterna sua visibilidade
  (`Cmd+Shift+B` no macOS).
- Manter a partida e as preferências ao navegar entre telas na mesma sessão.

A análise Stockfish, os sons, a busca no Chess.com, a importação por arquivo e a
persistência SQLite ainda não estão conectados. O modo **Demo · dados fictícios** permite visualizar avaliações,
acurácia, classificações, gráficos e estados de progresso/erro simulados. Importações
de PGN diferentes do exemplo abrem com o mock desligado; o switch permite optar
explicitamente pela demonstração. Os perfis e recursos são preferências da sessão;
seus orçamentos continuam pertencendo ao backend Tauri. Não há leitura nem alteração
do banco ou dos ajustes do app principal. A prévia PGN não substitui a revalidação do
backend quando essa integração for implementada.

## Layout responsivo

A janela suporta a partir de **720 × 560 pt**. O tabuleiro usa casas em pontos inteiros
e ajusta tamanho à largura e à altura disponíveis (até 640 pt). Com menos de
760 pt de área útil, o painel de revisão passa para baixo do tabuleiro; o botão
**Lances ↓** leva até ele. A sidebar pode ser recolhida ou redimensionada.

Formulários rolam em janelas baixas. Linhas de preferências quebram quando o texto
e o controle não cabem lado a lado; campos PGN têm rolagem própria. O corpo da
interface usa 13–14 pt, legendas têm no mínimo 12 pt e botões de ação têm 36 pt.

`check-responsive.sh` verifica 720×560, 900×700, 1024×768 e 1440×1080, incluindo
rolagem e clique nos controles inferiores em janelas pequenas.

## Verificar sem janela

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
./scripts/check-headless.sh
./scripts/check-responsive.sh
```

Os scripts renderizam as telas e os estados de interação em `artifacts/` e verificam
as árvores exportadas. Requer Python 3. Abra os PNGs para a inspeção visual.

```sh
cargo run -- --scene settings --snap artifacts/settings.png --tree artifacts/settings.json --scale 1
cargo run -- --scene settings-light --snap artifacts/settings-light.png --scale 1
cargo run -- --script scripts/review.json --at 3.8 --snap artifacts/interaction.png --scale 1
cargo run -- --script scripts/settings.json --at 4 --snap artifacts/settings-interaction.png --scale 1
```

Os artefatos gerados são ignorados pelo Git. Fontes IBM Plex e Noto Sans Symbols 2
incluídas com suas licenças OFL em `assets/`. A licença MIT do exemplo Rune está
em `RUNE-LICENSE.txt`. O experimento segue GPL-3.0-or-later, como o EngineRoom.


## Estado WIP

A revisão é um protótipo visual: não usa Stockfish, SQLite nem os comandos Tauri.
Os dados de avaliação, acurácia, classificação e fase são fixtures de apresentação;
não representam a qualidade real da partida. Variações e candidatos respeitam as
regras de xadrez. A validação visual dos novos painéis e a paridade fina com o app
principal continuam em andamento.
