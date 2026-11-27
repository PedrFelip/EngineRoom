# Fontes

- `IBMPlexSans-{Regular,Medium,SemiBold}.otf`: IBM Plex Sans para toda a interface,
  com pesos 400, 500 e 600.
- `IBMPlexMono-{Regular,Medium}.otf`: IBM Plex Mono para PGN e dados numéricos.
  Fontes IBM distribuídas pelo pacote `otf-ibm-plex`; projeto
  https://github.com/IBM/plex. Licença SIL OFL em `IBMPlex-LICENSE.txt`.

As fontes são registradas explicitamente para que o render nativo e headless
usem as mesmas famílias, sem depender das fontes instaladas no computador.
# Octicons

`sidebar-expand-16.svg`, `sidebar-collapse-16.svg`, `triangle-right-16.svg` e
`check-16.svg` são ícones Primer Octicons
do GitHub, sob licença MIT (incluída em `OCTICONS-LICENSE.txt`).
Fonte: https://github.com/primer/octicons/tree/main/icons

Board navigation uses Primer Octicons `move-to-start`, `chevron-left`,
`chevron-right`, `move-to-end`, and `arrow-switch` (16 px), sourced from
https://github.com/primer/octicons/tree/main/icons. MIT license in OCTICONS-LICENSE.txt.

Additional interface icons are Primer Octicons, sourced from https://github.com/primer/octicons/tree/main/icons and distributed under the included MIT license.

# Marca EngineRoom

Os SVGs editáveis ficam em [`docs/brand`](../../docs/brand). A barra lateral
usa `engineroom-logo.svg` no tema escuro e `engineroom-logo-light.svg` no
tema claro. O AppImage usa o PNG. O instalador NSIS e
o executável Windows usam o ICO com múltiplos tamanhos. Para regenerar os
ícones a partir da raiz do projeto:

```sh
rsvg-convert -w 256 -h 256 docs/brand/engineroom-icon.svg -o assets/icons/engineroom-rune.png
magick assets/icons/engineroom-rune.png -define icon:auto-resize=256,128,64,48,32,24,16 assets/icons/engineroom-rune.ico
```
