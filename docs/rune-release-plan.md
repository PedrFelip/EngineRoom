# Plano de empacotamento e release: Rune substitui o app Tauri

Escopo da implementação da Fase 3: todo o Rune permanece em `experimental/`,
com AppImage Linux e NSIS Windows; macOS e reorganização para a raiz ficam
fora desta etapa. O downloader permanece em `app/scripts/`. Comandos locais
em [releases.md](releases.md#rune-empacotamento-de-pré-releases).

Decisão registrada em 13/11/2026: o app Tauri (`app/`) será descontinuado e o
experimento Rune (`experimental/`) se torna o único produto desktop, organizado
na **raiz do repositório** (sem subpasta própria e sem versão web). Este
documento detalha as Fases 3 (empacotamento) e 5 (corte/release) do
[plano de migração](rune-migration-plan.md) com o novo destino na raiz.

## Empacotamento (Fase 3)

### Decisão de ferramenta

Comparação rápida:

- **`cargo-packager`**: gera AppImage, deb, NSIS, MSI, DMG e pkg a partir de
  uma config declarativa. Próximo do que `tauri build` faz hoje; mantém o
  pipeline declarativo e multiplataforma em um único arquivo.
- **`cargo-dist`**: foco em distribuição binária e artefatos zip/tar; AppImage
  e instaladores NSIS/MSI têm suporte parcial ou inexistente.
- **Scripts próprios**: máxima flexibilidade, mas reinventa o que
  `cargo-packager` já cobre e herda a manutenção dos três instaladores.

Proposta: **`cargo-packager` como empacotador principal**, com scripts de
verificação reaproveitados do Tauri (`app/scripts/verify_appimage.py` vira
`scripts/verify_appimage.py` na raiz) para manter a checagem de compatibilidade
gráfica (Wayland/EGL/GLES) que hoje bloqueia a release. `cargo-dist` só volta
à conversa se o empacotador declarativo não cobrir algum alvo.

### Sidecar Stockfish

O download continua único e compartilhado; nenhuma infraestrutura nova para o
experimento:

- `app/scripts/fetch-stockfish.mjs` é movido para `scripts/fetch-stockfish.mjs`
  na raiz (com o `.gitignore` de `binaries/` e o cache de CI atualizados).
- No empacotamento, o binário baixado é incluído **ao lado do executável**
  empacotado (AppImage `usr/bin`, NSIS/MSI install dir), nomeado
  `stockfish`+sufixo de plataforma, como o Tauri faz hoje com `externalBin`.
- `desktop_paths.rs` ganha uma etapa de resolução "diretório do executável"
  antes do fallback de desenvolvimento (`app/src-tauri/binaries/stockfish-*`
  e `ENGINE_ROOM_STOCKFISH`), para que o build empacotado encontre o motor sem
  variável de ambiente.
- O cache de CI do sidecar (`stockfish-v1-...`) muda o `path` para o novo
  local na raiz; a chave continua hash do script de fetch.

### Ordem de execução da Fase 3

1. Travar a decisão de plataformas da primeira release (única decisão
   `[decidir]` da Fase 0 que resta relevante aqui): Linux primeiro (onde o
   Rune já roda e já existe CI de AppImage) com Windows/macOS em seguida.
2. Config `cargo-packager` por plataforma + `scripts/verify_appimage.py`.
3. Empacotar o sidecar junto ao executável e ajustar `desktop_paths.rs` com
   teste headless (sem motor → mensagem de ausência, nunca crash).
4. Pipeline de empacotamento local documentado em `docs/releases.md` antes da
   automação.

## Releases (Fase 5)

- **Versionamento**: `release-please-config.json` ganha um bloco para o
  workspace Rune na raiz (`Cargo.toml` da raiz passa a ser o único app
  versionado; o do Tauri é removido do manifest no corte). O congelamento de
  versão do `release_version.py` é estendido para os paths novos.
- **Tag e draft**: mantém o fluxo atual — merge do PR do Release Please cria
  tag e draft de release; testes e builds de instaladores precisam passar
  antes da publicação automática; dispatch manual a partir da `main` retoma
  uma tag existente.
- **Gate**: o job `rune` do CI (fmt, clippy, testes, capturas headless) entra
  no gate `required` já nesta fase; o gate terminal continua falhando com
  skipped/failed de dependências.
- **Notas**: `docs/releases.md` é reescrito para o Rune (deps nativas de
  Vulkan/Mesa, resolução do sidecar empacotado) antes da primeira release.

## Migração de dados do usuário (pré-requisito do corte)

A Fase 4 do plano de migração continua bloqueando a primeira release pública:
migração de `engineroom.db`/preferências do formato Tauri
(`app_data_dir/engineroom.db`) para o formato experimental
(`engineroom-experimental/engineroom.db`), com teste automatizado
(fixture do banco antigo → abre no Rune → histórico aparece). Enquanto isso
não existir, os usuários do Tauri perdem cache/histórico no corte.
