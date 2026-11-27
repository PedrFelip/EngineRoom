# Releases

O EngineRoom distribui AppImage Linux x86_64 e NSIS Windows x86_64. As
atualizações são instaladas manualmente.

## Desenvolvimento e promoção

Use branches com títulos Conventional Commit e PRs para `develop`, com squash
merge. Promova `develop` para `main` com merge commit. Preserve o histórico
ao aceitar a sincronização automática de `main` para `develop`. Hotfixes
podem direcionar PRs para `main`.

O Release Please mantém a versão do pacote raiz, o workspace, `Cargo.lock` e
o changelog. O merge de seu PR cria uma tag e um draft. O workflow executa os
checks no commit imutável, constrói ambos os instaladores e publica somente se
todos os jobs passarem. O AppImage também é executado em Ubuntu 24.04 depois
do build no Ubuntu 22.04. A publicação é seguida pela sincronização de branches.
O gate terminal `CI passed` exige sucesso de todos os checks.

Para retomar uma release interrompida, execute o workflow Release manualmente
em `main`, informando uma tag existente. Tags avulsas e mudanças manuais de
versão não publicam uma release.

## Empacotamento local

Instale Rust estável, Node.js, Python 3.11+ e o empacotador fixado:

```sh
cargo install cargo-packager --version 0.11.8 --locked
python scripts/package.py
```

No Linux, instale compilador C/C++, pkg-config, bibliotecas de desenvolvimento
ALSA, udev, XKB, Wayland, EGL e Vulkan, Mesa Vulkan, patchelf, file,
squashfs-tools e libfuse2. Use um display ativo ou `xvfb-run` com Xauth.
Compile no Ubuntu 22.04 para manter compatibilidade com sua glibc e versões
posteriores. O teste extrai o AppImage, verifica as bibliotecas gráficas e o
Stockfish e inicia o aplicativo com Vulkan.

No Windows, execute o comando nativamente com Rust MSVC, Visual Studio Build
Tools, Windows SDK, Node e Python. O script verifica instalação silenciosa,
renderização inicial e handshake UCI do motor empacotado, depois desinstala a
cópia temporária. O aplicativo requer um driver DirectX 12 ou Vulkan.

Os pacotes ficam em `target/packages/` e incluem Stockfish 19, licença GPL e
avisos de terceiros. O workflow `Rune installers` permite verificar os dois
alvos sem publicar uma release.
