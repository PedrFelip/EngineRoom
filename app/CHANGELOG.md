# Changelog

## [2.2.0](https://github.com/PedrFelip/EngineRoom/compare/v2.1.4...v2.2.0) (2026-10-26)


### Funcionalidades

* **review:** pula entre erros e blunders com Shift e setas ([#36](https://github.com/PedrFelip/EngineRoom/issues/36)) ([7bb4b91](https://github.com/PedrFelip/EngineRoom/commit/7bb4b912bf88391db43476ecd9b678e24e5caad3))

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
