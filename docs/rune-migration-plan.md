# Plano de migração: Rune substitui o app Tauri

Objetivo: o frontend nativo em Rust/Rune (hoje em `experimental/`) se torna o
**único** produto distribuído do EngineRoom. O app Tauri (`app/`) é mantido
em paralelo só até o Rune atingir paridade funcional e de release, e então é
removido do repositório.

Este documento é o plano; nenhuma mudança estrutural foi feita ainda.
Decisões em aberto estão marcadas como **[decidir]**.

## Critério de "pronto para substituir"

O corte só acontece quando todos os itens abaixo forem verdadeiros ao mesmo
tempo, não um de cada vez:

1. Paridade funcional com `app/` nos fluxos documentados no README raiz
   (importar, configurar, revisar, histórico, preferências, busca Chess.com).
2. Pipeline de release própria publicando os mesmos alvos que hoje saem do
   Tauri (AppImage, NSIS, MSI, ou os substitutos definidos na Fase 3).
3. CI do Rune no mesmo nível de rigor do `checks.yml` atual: fmt, clippy,
   testes com Stockfish real, scripts headless/responsive, tudo bloqueando o
   merge.
4. Caminho de migração de dados do usuário (cache SQLite, histórico,
   preferências) do formato do app Tauri para o formato do Rune, testado.
5. Documentação (README raiz, `docs/releases.md`) reescrita para o novo app,
   sem seção "experimental".

Enquanto isso não for verdade, `app/` continua sendo o app publicado.

## Fases

### Fase 0 — Decisões de produto **[bloqueante]**

- **[decidir]** Nome final do produto/pasta: mantém "EngineRoom" e só troca o
  motor de UI, ou também renomeia o app? Isso afeta app id, ícones, nome do
  instalador e possivelmente o nome do repositório. **Resolvido em
  13/11/2026:** mantém "EngineRoom" e todo o app Rune passa a viver na raiz do
  repositório (sem subpasta própria). Detalhes de empacotamento e release em
  [rune-release-plan.md](rune-release-plan.md).
- **[decidir]** Plataformas-alvo da primeira release do Rune: só Linux
  primeiro (onde já roda e tem CI de AppImage), com Windows/macOS depois, ou
  as três de uma vez como hoje?
- **[decidir]** O que acontece com o histórico/cache de usuários que já têm
  o app Tauri instalado: migração automática na primeira abertura do Rune,
  ferramenta manual de importação, ou início limpo (perda documentada)?

### Fase 1 — Reorganização de diretório

- Mover `experimental/` → `app-rune/` (ou nome escolhido na Fase 0) na raiz,
  com `git mv` para preservar histórico. Enquanto os dois apps coexistirem
  durante a transição, evitar o nome `app/` para não colidir com o Tauri.
- Atualizar `README.md` raiz, `AGENTS.md`, scripts (`check-headless.sh`,
  `check-responsive.sh`, `check-review-matrix.py`) e variáveis de ambiente
  (`ENGINE_ROOM_STOCKFISH`) para os novos caminhos. Alvo decidido: raiz do
  repositório (ver [rune-release-plan.md](rune-release-plan.md)).
- Manter a seção do README raiz explicando que `app/` está em descontinuação
  e será removido quando o Rune atingir os critérios da Fase 0.

### Fase 2 — CI em paralelo, sem derrubar o gate atual

- Adicionar um job `rune` em `.github/workflows/checks.yml`: `cargo fmt
  --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --workspace --locked`, scripts headless/responsive.
- Incluir esse job no gate `required` de `ci.yml` assim que estabilizar (sem
  flakiness), igual ao job `rust` do app Tauri hoje.
- Manter os jobs do app Tauri intactos nessa fase — o objetivo aqui é dar
  cobertura ao Rune, não reduzir a do app atual.

### Fase 3 — Empacotamento e release do Rune

Esse é o maior gap técnico: não existe bundler equivalente ao `tauri build`
para uma janela nativa Rune.

- **[decidir]** Ferramenta de empacotamento: `cargo-dist`, `cargo-packager`,
  ou scripts próprios reaproveitando a lógica de AppImage/NSIS/MSI que hoje
  vive nos workflows e em `app/scripts/verify_appimage.py`.
- Portar a verificação de compatibilidade gráfica do AppImage (Wayland/EGL/
  GLES, descrita em `docs/releases.md`) para o novo pipeline, já que o Rune
  também depende de Vulkan/Mesa e pode ter o mesmo tipo de problema de libs
  do sistema.
- Resolver o download/bundling do sidecar Stockfish de forma independente do
  app Tauri (hoje o Rune só funciona em dev reaproveitando
  `app/src-tauri/binaries/stockfish-*` ou uma variável de ambiente).
- Adicionar o pacote do Rune ao `release-please-config.json` (novo bloco em
  `packages`, nos moldes do bloco `"app"` atual) e ao
  `.release-please-manifest.json`, com `extra-files` apontando para os
  `Cargo.toml`/`Cargo.lock` corretos do novo workspace.

### Fase 4 — Migração de dados do usuário

- Implementar o caminho decidido na Fase 0 para `engineroom.db` e
  preferências: do formato/local do app Tauri
  (`app_data_dir`/`engineroom.db`) para o formato/local atual do
  experimental (`engineroom-experimental/engineroom.db`), unificando no
  nome final do produto.
- Cobrir a migração com teste automatizado (fixture do banco antigo → abre
  no Rune → dados aparecem no histórico).

### Fase 5 — Corte

Só quando os 5 critérios da seção anterior estiverem satisfeitos:

- Trocar o app publicado: `release.yml` passa a empacotar o Rune, não mais
  o Tauri.
- Remover `app/` do repositório (código Tauri/React) e seus workflows
  específicos, ou movê-los para uma branch/tag de arquivamento se quiser
  preservar histórico acessível.
- Reescrever o README raiz e `docs/releases.md` para descrever só o Rune,
  removendo a narrativa "experimento"/"estudo independente".
- Primeira release pública do Rune via Release Please, confirmando que o
  manifest e o changelog ficam corretos sem o pacote antigo.

## Riscos e pontos de atenção

- **Dependências do Rune fixadas por commit hash** (não tag/versão) —aceitável
  em experimento, arriscado para a release final. Negociar uma tag/versão
  estável antes da Fase 5.
- **Empacotamento nativo sem Tauri** é trabalho não trivial e é o maior risco
  de cronograma do plano.
- **Dois pipelines de revisão em Rust** (`app/src-tauri/src/review/` e
  `experimental/crates/engineroom-{domain,application,...}`) já existem
  separados; mesmo depois do corte, avaliar se vale extrair um crate único
  compartilhado em vez de manter a duplicação que a migração herdar.
- **Usuários existentes do app Tauri** perdem o instalador antigo no corte;
  a comunicação da migração de dados (Fase 4) precisa estar pronta antes da
  Fase 5, não depois.

## Não escopo deste plano

- Mudanças de produto/UX do Rune em si (já cobertas pelo README do
  experimental).
- Qualquer decisão de manter os dois apps publicados simultaneamente — isso
  foi descartado: o objetivo confirmado é substituição total.
