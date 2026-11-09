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

O backend nativo está dividido nas crates deste workspace:

- `crates/engineroom-domain`: tipos de análise, modo, transformações de xadrez, pontuação e perfis adaptativos.
- `crates/engineroom-application`: portas de motor e repositório, pipeline, pool, sessões e normalização de revisões.
- `crates/engineroom-uci`: adaptador do processo Stockfish via UCI.
- `crates/engineroom-sqlite`: cache, histórico, migrações e estatísticas em SQLite.
- `crates/engineroom-backend`: fachada de revisão e importações HTTP nativas.

- `src/main.rs`: composição das telas, cenas headless e atalhos.
- `src/state.rs`: navegação tipada, rascunho PGN, importações e estado da sessão.
- `src/bridge.rs`: runtime Tokio, intenções, sessões, histórico, armazenamento e importações.
- `src/review_session.rs`: identificação e sequência da sessão de análise.
- `src/preferences.rs`: documento versionado, escrita atômica e limites validados.
- `src/file_dialog.rs`: seletor nativo de arquivos sem bloquear a interface.
- `src/desktop_paths.rs`: diretório experimental e resolução do Stockfish.
- `src/theme.rs`: paleta OpenCode (claro/escuro), escala tipográfica e fontes IBM Plex.
- `src/layout.rs`: medidas responsivas baseadas na área útil real do shell.
- `src/game.rs`: validação local de PGN/FEN e posições para a prévia.
- `src/components/ui.rs`: grupos, linhas de preferência, tipografia e slider acessível.
- `src/components/shell.rs`: shell sem faixa superior, `Sidebar` e `SidebarRow` do kit.
- `src/components/home.rs`: `TextArea`, perfis segmentados e importação/configuração.
- `src/components/board.rs`: tabuleiro e identificação dos jogadores.
- `src/components/review.rs`: layout, playback e momentos críticos da análise.
- `src/components/review_{analysis,feedback,graph,summary}.rs`: painéis e gráficos da revisão.
- `src/review_data.rs`: formatação de avaliações, classificações e linhas candidatas.
- `src/fixtures.rs`: dados determinísticos usados somente por cenas headless e testes.
- `src/variation.rs`: exploração de lances legais sem alterar a linha principal.
- `src/components/history.rs`: histórico paginado do SQLite experimental.
- `src/components/settings.rs`: Preferências, Análise e Armazenamento.

Referências visuais: [Settings](https://github.com/anomalyco/rune/blob/dev/demo/settings-dark.png)
e [OpenCode](https://github.com/anomalyco/rune/tree/dev/demo/opencode).
A documentação de [rune-kit](https://github.com/anomalyco/rune/tree/dev/crates/rune-kit)
orienta a composição; a [documentação de Rune](https://github.com/anomalyco/rune/blob/dev/crates/rune/README.md)
orienta layout, escopos de eventos, acessibilidade e testes headless.

## O que funciona

- Colar e editar um PGN; validar lances legais, FEN de início e uma partida por importação.
- Importar por caminho de arquivo ou pelo seletor nativo; quando o seletor não está
  disponível, o campo de caminho continua funcionando como fallback.
- Buscar partidas no Chess.com por usuário com HTTP nativo, paginação e limites de
  arquivos, candidatos, resposta, PGN e tempo.
- Na importação Chess.com, tentar carregar em segundo plano as fotos públicas dos jogadores para a revisão. Quando indisponíveis, mostrar iniciais; as fotos obtidas são guardadas localmente e restauradas ao abrir o histórico, mesmo offline; fotos idênticas são compartilhadas entre partidas.
- Configurar um perfil na interface e abrir a prévia, preservando o rascunho ao voltar.
- Navegar por clique, setas, Home/End e Shift+setas entre momentos críticos.
- Alternar Lances, Análise e Resumo; filtrar erros por cor; clicar no gráfico para navegar.
- Visualizar barra de avaliação, selos no tabuleiro/lista, setas e resumo por fase da análise.
- Selecionar candidatos legais, reproduzir uma linha, pausar e retornar à partida.
- Criar variações por clique ou arraste de peças (promoção automática para dama).
- Configurar linhas, feedback, tempo, threads e memória no modal da revisão.
- Alternar foco no tabuleiro e painéis, inclusive com a sidebar fechada.
- Trocar tema em toda a interface; mudar switches, sliders e linhas candidatas.
- Recolher e redimensionar a sidebar; `Ctrl+B` alterna sua visibilidade
  (`Cmd+B` no macOS).
- Recolher os painéis da revisão com `Ctrl+Alt+B` (Control também no macOS)
  ou pelo botão Octicon no cabeçalho; largura entre 304 e 392 pt por arraste
  ou teclado na divisória, com duplo clique para restaurar 352 pt.
  Aba, lance e largura são mantidos ao ocultar e ao navegar entre telas.
- Alternar entre sessões abertas pela dock, preservando lance, orientação, variações e resultado.
  Uma análise fica ativa por vez; ao trocar, a anterior é encerrada e a revisão
  incompleta pode ser retomada pelo cache. Fechar uma sessão não apaga seu histórico.
- Reabrir, reanalisar e excluir revisões salvas na lista paginada do histórico
  ("Carregar mais").
- Persistir as preferências em `engineroom-experimental/preferences.json` dentro do
  diretório de dados do usuário (no Linux, `$XDG_DATA_HOME`); a seção Análise envia
  linhas, tempo, threads e memória ao motor ao vivo.
- Ver estatísticas reais do SQLite experimental e limpar o cache; o histórico só é
  apagado após confirmação explícita.

A análise nativa usa Stockfish e o SQLite experimental
`engineroom-experimental/engineroom.db` no diretório de dados do usuário (no Linux,
`$XDG_DATA_HOME`), separado do banco do aplicativo principal. Sons e áudio ainda não estão conectados, e o
empacotamento/release do Rune segue pendente. A interface usa os resultados reais do backend, sem controles de simulação. Os cenários headless permanecem determinísticos e não abrem Stockfish nem banco.

## Layout responsivo

A janela suporta a partir de **720 × 560 pt**. O tabuleiro usa casas em pontos inteiros
e a revisão distribui navegação, partida e ferramentas em três regiões: as duas
barras compartilham a superfície do shell; o tabuleiro fica no painel central.
Em desktop, a barra direita ocupa a altura toda e seu conteúdo rola separadamente.
Lances acompanha a posição selecionada, Análise explora suas alternativas e
Resumo apresenta a partida inteira; o feedback do lance fica nas duas primeiras abas.
O tabuleiro ajusta tamanho à largura e à altura disponíveis, em casas de pontos inteiros.
Quando não cabe um tabuleiro de 320 pt ao lado do inspector, a navegação persistente
alterna entre Tabuleiro, Lances, Análise e Resumo. Os painéis usam a altura útil e
rolagem própria; os controles de lances permanecem visíveis. A sidebar pode ser recolhida ou redimensionada.

Formulários rolam em janelas baixas. Linhas de preferências quebram quando o texto
e o controle não cabem lado a lado; campos PGN têm rolagem própria. O corpo da
interface usa 13–14 pt, legendas têm no mínimo 12 pt. Botões de ação usam 36 pt; a navegação
compacta do tabuleiro mantém alvos de 32 pt com nomes acessíveis e atalhos.

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

A suíte headless inclui uma matriz de 160 capturas da revisão: oito cenas
(temas, abas, variação, progresso, erro e prévia), quatro tamanhos e cinco
combinações de visibilidade dos painéis. Para cada caso, guarda PNG, árvore,
script reproduzível e log em `artifacts/review-matrix/`. Os relatórios `report.md`
e `report.json` registram falhas por caso, duração, geometria do tabuleiro/painel,
quantidade de elementos e nós acessíveis, controles focáveis e foco atual.
A coleta continua após falhas e termina com código diferente de zero se houver
qualquer erro. Verifica também sobreposição, 64 casas e controles acessíveis.

Para executar somente a matriz após `cargo build --locked`:

```sh
python3 scripts/check-review-matrix.py
python3 scripts/check-review-matrix.py --quick # 80 casos, janela mínima e ampla
python3 scripts/check-review-matrix.py --quick --analyze-only # revalidar árvores existentes
python3 scripts/test-headless-report.py # testar os diagnósticos sem renderizar
```

Os casos usam `Ctrl+Alt+B` para ocultar o painel, o botão Octicon para reabrir
e `Ctrl+B` para a barra esquerda. Os testes Rust também verificam escopo do
atalho, modificadores extras, modal e preservação do estado da revisão.

```sh
cargo run -- --scene settings --snap artifacts/settings.png --tree artifacts/settings.json --scale 1
cargo run -- --scene settings-light --snap artifacts/settings-light.png --scale 1
cargo run -- --script scripts/review.json --at 3.8 --snap artifacts/interaction.png --scale 1
cargo run -- --script scripts/settings.json --at 4 --snap artifacts/settings-interaction.png --scale 1
```

Os artefatos gerados são ignorados pelo Git. Fontes IBM Plex incluídas com sua licença OFL em `assets/`. A licença MIT do exemplo Rune está
em `RUNE-LICENSE.txt`. O experimento segue GPL-3.0-or-later, como o EngineRoom.


## Estado WIP

O frontend experimental ainda não está empacotado nem distribuído; som/áudio não
está conectado e a paridade visual com o aplicativo principal continua em
andamento. A interface não expõe controles de demonstração: partidas analisadas usam
Stockfish e o SQLite experimental. Fixtures de apresentação existem apenas nas cenas
headless (`--scene`, `--snap`, `--tree`), que não abrem motor nem banco. Variações e
candidatos respeitam as regras de xadrez.

### Análise nativa experimental

Para analisar um PGN importado, defina
`ENGINE_ROOM_STOCKFISH=/caminho/para/stockfish`, coloque-o em
`experimental/resources/stockfish` ou deixe o sidecar baixado para o aplicativo
principal em `app/src-tauri/binaries/stockfish-*` (desenvolvimento). Execute
`cargo run` a partir de `experimental/`, importe ou cole o PGN, escolha o perfil e
abra a revisão. O cache
e o histórico usam `engineroom-experimental/engineroom.db` no diretório de dados
do usuário (no Linux, `XDG_DATA_HOME` ou `~/.local/share`), separado do aplicativo
principal. A ausência do motor é
mostrada ao iniciar análise, sem impedir a abertura do aplicativo ou do histórico.
O histórico carrega páginas de até 100 revisões com "Carregar mais"; reabrir,
reanalisar e excluir usam o banco experimental. As preferências ficam no
`preferences.json` ao lado do banco. Os cenários headless mantêm fixtures determinísticas e não abrem o motor nem o banco.
