# EngineRoom

Revise suas partidas de xadrez com **Stockfish 19**, direto no computador.
Importe um PGN, analise os lances e explore as alternativas no tabuleiro.
A análise é local: seu PGN não é enviado a um servidor.

## Como funciona

1. **Importe a partida** arrastando um arquivo `.pgn` ou `.txt`, usando o
   seletor de arquivos, colando o PGN ou buscando pelo nome de usuário na
   aba **Chess.com**. Selecionar uma partida já preenche a prévia, sem um
   botão extra de importação.
2. **Escolha a análise.** O modo automático oferece os perfis Rápido e
   Profundo: avalia todas as posições e dedica mais tempo às decisões críticas.
   No modo manual, você define profundidade ou tempo por posição e o número
   de linhas candidatas.
3. **Revise lance a lance.** Veja a avaliação, o melhor lance sugerido,
   as linhas candidatas e a classificação dos lances. O resumo traz a
   acurácia de cada lado e a abertura identificada (ECO).
4. **Explore e retome.** Navegue pelo gráfico ou pela lista de lances,
   jogue variações no tabuleiro e reabra partidas pelo histórico.

As posições analisadas ficam em um cache local para reaproveitar resultados
compatíveis em novas revisões. O histórico permite reabrir, reanalisar e
excluir partidas.

A busca no Chess.com usa a API pública, sem login, e exige internet. Carrega
até 20 partidas concluídas de xadrez padrão por consulta, com **Buscar
anteriores** para continuar (até 100 na lista). Cada consulta percorre até
três arquivos mensais e tem limites de tempo e tamanho; resultados parciais
são indicados na interface. Partidas recém-terminadas podem demorar para
aparecer na API. Depois da importação, a análise continua local e offline.
O último username pesquisado com sucesso fica salvo neste dispositivo; ao
abrir a aba, suas partidas são buscadas automaticamente. Você pode trocar ou
esquecer o usuário salvo, filtrar a lista carregada por ritmo e ver os
resultados de sua perspectiva (vitória, derrota ou empate).

A importação tem duas etapas e preserva o rascunho ao voltar da revisão.
Os painéis de lances, análise e resumo dão mais espaço ao tabuleiro; atalhos
levam aos erros da partida. O histórico permite buscar jogadores nas partidas
carregadas, e as configurações são organizadas por seção.

**Atalhos na revisão:** `←` / `→` para o lance anterior ou seguinte;
`Home` / `End` para o início ou fim da partida. A interface tem temas claro
e escuro.

## Rodar o projeto

Você precisa de [Bun](https://bun.sh), Node.js, Rust com Cargo e das
[dependências de sistema do Tauri 2](https://tauri.app/start/prerequisites/).
O download inicial do Stockfish exige internet; depois disso, a revisão
funciona offline.

Execute os comandos abaixo em `app/`:

```bash
bun install
node scripts/fetch-stockfish.mjs
bun run tauri dev
```

O script baixa o Stockfish 19 universal para a plataforma atual, atualiza
sidecars de versões anteriores e pula o download se a versão correta já
estiver instalada. O Tauri inicia o Vite automaticamente.

Para gerar o aplicativo e os instaladores nativos, também em `app/`:

```bash
bun run tauri build
```

## Arquitetura

**Rust cuida da revisão; React cuida da interface.** O aplicativo usa Tauri 2
para conectar as duas partes.

| Parte | Responsabilidade | Código |
| --- | --- | --- |
| Análise | PGN, regras de xadrez, classificação, acurácia, aberturas e seleção adaptativa | [`review/`](app/src-tauri/src/review/) |
| Motor e sessões | Comunicação UCI com o Stockfish, fila de trabalho, análise ao vivo e encerramento dos processos | [`review/session/`](app/src-tauri/src/review/session/) |
| Persistência | Cache de posições, histórico e migrações SQLite | [`db/`](app/src-tauri/src/db/) |
| Importação online | API Chess.com, validação e buscas limitadas por janela | [`imports/`](app/src-tauri/src/imports/) e [`chess-com-import.ts`](app/src/lib/chess-com-import.ts) |
| Integração frontend | IPC com Effect, validação de eventos e ciclo de vida da sessão | [`review-session.ts`](app/src/lib/review-session.ts) e [`review-protocol.ts`](app/src/lib/review-protocol.ts) |
| Estado da revisão | Navegação e variações no tabuleiro | [`review-store.ts`](app/src/lib/review-store.ts) |

O backend revalida o PGN e respeita posições iniciais definidas por FEN.
A interface usa `chess.js` na prévia de importação e nas interações do
tabuleiro, renderizado com Chessground.

O `Pipeline` coordena a análise usando os contratos `EngineFactory` e
`Repository`. A aquisição, configuração e limpeza da engine ficam em
`pipeline/owned_engine.rs`; os comandos Tauri ficam em `session/ipc.rs`.
O adaptador SQLite fica em `repository/sqlite.rs`, separado da validação e
normalização de revisões salvas em `repository/saved_review.rs`.

A análise automática divide os recursos entre até três processos Stockfish;
a manual usa um. As sessões têm acesso exclusivo ao motor e aguardam a
limpeza dos processos ao encerrar. Na análise ao vivo, o pedido de navegação
mais recente tem prioridade e eventos antigos são descartados.

Os orçamentos dos perfis automáticos ficam em
[`profiles.json`](app/src-tauri/src/review/profiles.json), compartilhado pelo
backend e pela interface. O cache considera profundidade alcançada, orçamento
de tempo e cobertura de linhas candidatas para decidir o que pode reutilizar.

## Verificação

Em `app/`, verifique o frontend:

```bash
bun run lint && bun run typecheck && bun run test
```

Em `app/src-tauri/`, verifique o backend:

```bash
cargo test
```

Os testes cobrem o núcleo, o pipeline, as sessões e a persistência com engines
falsas, além da integração com o Stockfish real. Essa integração exige o
sidecar baixado no setup.

## Desenvolvimento e releases

Use branches de funcionalidade com PR para `develop`; promova mudanças prontas
para `main` preservando os commits. O bot prepara um PR com versão e changelog.
Ao fazer merge desse PR, os testes e builds geram e publicam os instaladores
para Windows e Linux automaticamente.

Veja o [guia de branches, commits e releases](docs/releases.md) para os tipos de
merge, Conventional Commits, sincronização e retomada de builds.

## Licença

GPL-3.0-or-later. Veja [LICENSE](LICENSE) e os
[avisos de terceiros](THIRD_PARTY_NOTICES.md).
