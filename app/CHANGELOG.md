# Changelog

## [2.3.2](https://github.com/PedrFelip/EngineRoom/compare/v2.3.1...v2.3.2) (2026-10-29)


### Correções

* handle special characters in Stockfish setup paths ([#56](https://github.com/PedrFelip/EngineRoom/issues/56)) ([d6a3f2c](https://github.com/PedrFelip/EngineRoom/commit/d6a3f2c976739a9b112a97c1fd4f47e297d6abdf))
* **linux:** corrige reprodução dos sons no AppImage ([#64](https://github.com/PedrFelip/EngineRoom/issues/64)) ([db11dee](https://github.com/PedrFelip/EngineRoom/commit/db11dee23d4330132efbac94c2b5649abeed8b6c))
* resume interrupted Chess.com import search ([#58](https://github.com/PedrFelip/EngineRoom/issues/58)) ([3bb66f5](https://github.com/PedrFelip/EngineRoom/commit/3bb66f5ee160498fb7bf443e4689643e0bc35a6c))
* retain partial Chess.com results at deadline ([#59](https://github.com/PedrFelip/EngineRoom/issues/59)) ([860f4cf](https://github.com/PedrFelip/EngineRoom/commit/860f4cfa8c9b307b8db4bb71d0cbca3da3e99659))
* reuse valid covering cache entries ([#60](https://github.com/PedrFelip/EngineRoom/issues/60)) ([e68408c](https://github.com/PedrFelip/EngineRoom/commit/e68408cee48e4c86ba4b6f003107260c248a6c6e))

## [2.3.1](https://github.com/PedrFelip/EngineRoom/compare/v2.3.0...v2.3.1) (2026-10-29)


### Correções

* **Linux:** corrige a reprodução de áudio no AppImage, incluindo os componentes necessários para tocar MP3 e enviar o som ao sistema. ([c3f0e2f](https://github.com/PedrFelip/EngineRoom/commit/c3f0e2f0ddb07db35e4c3fbc10c2af61a80bf453))

## [2.3.0](https://github.com/PedrFelip/EngineRoom/compare/v2.2.0...v2.3.0) (2026-10-29)


### Funcionalidades

* adiciona atualizações pelo aplicativo e seção Sobre ([#45](https://github.com/PedrFelip/EngineRoom/issues/45)) ([b70b98d](https://github.com/PedrFelip/EngineRoom/commit/b70b98d374ad3adaa15184fe37e556156f39bfd4))
* **review:** add F shortcut to flip the board ([#47](https://github.com/PedrFelip/EngineRoom/issues/47)) ([65b6a06](https://github.com/PedrFelip/EngineRoom/commit/65b6a0632d6ae4a19e63a7c6ad304790e8506d65))


### Correções

* seleciona o instalador correto para atualizações ([#48](https://github.com/PedrFelip/EngineRoom/issues/48)) ([44d445d](https://github.com/PedrFelip/EngineRoom/commit/44d445d98eaafdd763b60c1a34cc3bfc6a8c3a80))
* **ui:** padroniza controles e estados de interação ([#46](https://github.com/PedrFelip/EngineRoom/issues/46)) ([266c450](https://github.com/PedrFelip/EngineRoom/commit/266c450511618eb6082d05b9a4586af9a2ac0fd4))

## [2.2.0](https://github.com/PedrFelip/EngineRoom/compare/v2.1.4...v2.2.0) (2026-10-26)


### Novidades

- Na revisão, use **Shift + ←** e **Shift + →** para navegar entre erros e blunders. Os atalhos também funcionam com o gráfico em foco e respeitam a linha principal ao sair de uma variação. ([#36](https://github.com/PedrFelip/EngineRoom/pull/36))
- A análise em Rust foi reorganizada para separar as responsabilidades da engine, das sessões e da persistência, preservando o comportamento e a comunicação do aplicativo. ([#38](https://github.com/PedrFelip/EngineRoom/pull/38))

Agradecemos a [@gabbzin](https://github.com/gabbzin) pela primeira contribuição ao EngineRoom! 🎉

## [2.1.4](https://github.com/PedrFelip/EngineRoom/compare/v2.1.3...v2.1.4) (2026-10-26)


### Correções

* **review:** harden edge cases across analysis and shutdown ([ad54835](https://github.com/PedrFelip/EngineRoom/commit/ad54835c9f74f16ab7952629760f47d2d694d171))
* **review:** harden edge cases across analysis and shutdown ([77820e2](https://github.com/PedrFelip/EngineRoom/commit/77820e215a41d0c3a7c59c2fcb21455add05e286))

## [2.1.3](https://github.com/PedrFelip/EngineRoom/compare/v2.1.2...v2.1.3) (2026-10-25)


### Correções

* migra frontend e corrige ambiente de teste do AppImage ([6841bab](https://github.com/PedrFelip/EngineRoom/commit/6841babed85b321e1007ef4ceb9bd65f50e8fc07))

## [2.1.2](https://github.com/PedrFelip/EngineRoom/compare/v2.1.1...v2.1.2) (2026-10-25)


### Correções

* atualiza stack do aplicativo para 2.1.2 ([abf8bde](https://github.com/PedrFelip/EngineRoom/commit/abf8bde2551941f72a8881cabc68e41f78e3417e))
* atualiza stack e corrige AppImage no Linux ([80cc646](https://github.com/PedrFelip/EngineRoom/commit/80cc646a7473c8935347558f38e17ea0eeb21f7a))
* usa empacotador corrigido do Tauri para AppImage ([f43c31e](https://github.com/PedrFelip/EngineRoom/commit/f43c31e7ccbd7e2975cee98719572b68a860795f))

## [2.1.1](https://github.com/PedrFelip/EngineRoom/compare/v2.1.0...v2.1.1) (2026-10-25)


### Correções

* inclui empacotamento do AppImage na release 2.1.1 ([9a9a329](https://github.com/PedrFelip/EngineRoom/commit/9a9a329d134c4781b1c1724a7ebf16d8368b82bf))
* inclui empacotamento do AppImage na release 2.1.1 ([31aa436](https://github.com/PedrFelip/EngineRoom/commit/31aa436e3be82050e7e2f3c66376c37b9017397f))

## [2.1.0](https://github.com/PedrFelip/EngineRoom/compare/v2.0.0...v2.1.0) (2026-10-25)


### Funcionalidades

* add Chess.com game import and harden Stockfish download ([f894dbb](https://github.com/PedrFelip/EngineRoom/commit/f894dbb3b947a15b2ec49d92ed5635f678a6d27b))
* importa partidas do Chess.com para revisão local ([7e5f864](https://github.com/PedrFelip/EngineRoom/commit/7e5f86429100f17adcbfa5141e17900af79abd38))
* **ui:** melhora navegação e ícones da importação Chess.com ([1f15a7d](https://github.com/PedrFelip/EngineRoom/commit/1f15a7d3de9cc3ec1b80ba772ca8bf63251607e0))

## [2.0.0](https://github.com/PedrFelip/EngineRoom/compare/v1.0.0...v2.0.0) (2026-10-23)


### ⚠ BREAKING CHANGES

* **ui:** a nova interface reorganiza a preparação, a revisão e as configurações das partidas e inaugura o EngineRoom 2.0.

### Funcionalidades

* **ui:** apresenta a nova interface do EngineRoom 2.0 ([de172d5](https://github.com/PedrFelip/EngineRoom/commit/de172d5abb323733d7ec69dbc78eb14cd3a9c178))
* **ui:** dá mais espaço ao tabuleiro com painéis de lances, análise e resumo e atalhos para os momentos críticos ([de172d5](https://github.com/PedrFelip/EngineRoom/commit/de172d5abb323733d7ec69dbc78eb14cd3a9c178))
* **ui:** facilita retomar partidas com busca por jogador no histórico e confirmação antes de excluir ([de172d5](https://github.com/PedrFelip/EngineRoom/commit/de172d5abb323733d7ec69dbc78eb14cd3a9c178))
* **ui:** moderniza a barra e o gráfico de avaliação com valores mais legíveis e navegação por teclado ([de172d5](https://github.com/PedrFelip/EngineRoom/commit/de172d5abb323733d7ec69dbc78eb14cd3a9c178))
* **ui:** organiza preferências, análise e armazenamento em configurações por seção, com navegação por teclado ([de172d5](https://github.com/PedrFelip/EngineRoom/commit/de172d5abb323733d7ec69dbc78eb14cd3a9c178))
* **ui:** simplifica a importação em duas etapas e preserva a partida e os ajustes ao voltar da revisão ([de172d5](https://github.com/PedrFelip/EngineRoom/commit/de172d5abb323733d7ec69dbc78eb14cd3a9c178))
* **ui:** unifica o visual de painéis e controles e adapta a interface a janelas menores ([de172d5](https://github.com/PedrFelip/EngineRoom/commit/de172d5abb323733d7ec69dbc78eb14cd3a9c178))


### Correções

* **ui:** elimina piscadas e deslocamentos ao alternar abas e mantém o gráfico estável ([de172d5](https://github.com/PedrFelip/EngineRoom/commit/de172d5abb323733d7ec69dbc78eb14cd3a9c178))

## [1.0.0](https://github.com/PedrFelip/EngineRoom/compare/v0.9.1...v1.0.0) (2026-10-23)


### Funcionalidades

* **ci:** automatiza versões e releases ([5f9214d](https://github.com/PedrFelip/EngineRoom/commit/5f9214db3f96c8ec06385574dca421ca25ffbffa))
* **ci:** automatiza versões e releases ([4d2d207](https://github.com/PedrFelip/EngineRoom/commit/4d2d20710e9072bb54273655f9c7630355ba2010))
* teste ci ([1e3319d](https://github.com/PedrFelip/EngineRoom/commit/1e3319d261356074d19da24886bd3668ded5799c))

## Changelog

As próximas versões são preparadas automaticamente a partir de Conventional
Commits e revisadas no PR de release. O histórico anterior está nas
[releases do GitHub](https://github.com/PedrFelip/EngineRoom/releases).
