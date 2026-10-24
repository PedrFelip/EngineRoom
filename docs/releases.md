# Desenvolvimento e releases

Fluxo: `feature/* → develop → main → PR de versão → release pública`.

## Trabalho diário

1. Crie `feature/*` ou `fix/*` a partir de `develop`.
2. Abra um PR para `develop` com título Conventional Commits. Faça **Squash and merge**
   usando título e descrição do PR como mensagem do commit.
3. Para preparar uma versão, abra `develop → main`, com título
   `chore: promove develop para main`. Use **Create a merge commit**: os commits
   das funcionalidades e correções precisam chegar à main para gerar o changelog.
4. O Release Please abre ou atualiza um PR de versão na main. Revise as notas e
   faça **Squash and merge** quando quiser publicar.
5. O Actions valida o commit da versão, gera AppImage, NSIS e MSI e publica a release.
6. Faça merge do PR automático `main → develop` com **Create a merge commit**.

Para uma correção urgente, crie `hotfix/*` a partir da main e abra um PR para main.
Use um título `fix: ...`, faça squash e siga o mesmo PR automático de versão.
Sincronize o resultado de volta para develop.

Não exclua `develop` ou `main` depois dos merges. Ambas exigem PR e o check
`CI passed`, inclusive para administradores. Como o projeto permite trabalho solo,
não há exigência de aprovação por uma segunda pessoa.

## Versões e changelog

| Título do PR | Incremento após 1.0.0 |
| --- | --- |
| `fix: corrige importação` | patch, como 1.0.1 |
| `perf: reduz tempo de análise` | patch |
| `feat: adiciona exportação` | minor, como 1.1.0 |
| `feat!: altera formato incompatível` | major, como 2.0.0 |
| `docs:`, `test:`, `chore:`, `ci:`, `build:`, `refactor:` | não geram release isoladamente |

Scopes são opcionais: `fix(review): corrige navegação`. Uma quebra também pode ser
descrita no rodapé `BREAKING CHANGE: ...`; preserve esse rodapé na mensagem do squash.
Escreva descrições úteis para quem usa o app: elas aparecem nas notas públicas.

O PR de versão atualiza `app/CHANGELOG.md`, o manifest do Release Please e as quatro
versões do app: package.json, tauri.conf.json, Cargo.toml e a entrada local no Cargo.lock.
Não altere essas versões manualmente. O seletor TOML do Cargo.lock usa `name.value`
porque a versão fixada do Release Please representa strings TOML como valores anotados;
os testes exercitam os updaters reais e preservam todas as dependências.

A implantação inicial contém `Release-As: 1.0.0` na mensagem do commit, após uma
linha em branco. Esse rodapé força apenas a primeira versão. O ponto inicial do
histórico é a tag `v0.9.1`; as releases anteriores são preservadas.

## CI e permissões

`CI` executa em PRs e pushes para main/develop. Inclui lint, typecheck, testes do
frontend, `cargo test --locked` com Stockfish real e testes dos scripts de release.
A política do PR é validada antes dos jobs de testes; uma falha bloqueia o gate
`CI passed`, inclusive quando os testes são pulados. O evento `edited` revalida o
título Conventional Commits. Pushes validam o resultado integrado em main/develop;
a release repete os checks no seu SHA imutável antes de gerar instaladores.

O cache global de pacotes Bun é separado por sistema, arquitetura, versão do Bun
e lockfile, com reaproveitamento dos pacotes ao mudar o lockfile. A instalação
continua usando `--frozen-lockfile`. Rust mantém o cache de dependências e compilação
por job, separando testes (debug) dos instaladores (release). O job Rust salva
esse cache mesmo quando um teste falha, para reaproveitar a compilação na próxima
execução. A instalação das bibliotecas Linux dos testes usa
`--no-install-recommends`: mantém dependências obrigatórias e evita os pacotes
recomendados. Ayatana AppIndicator e libxdo não são instalados nesse job porque
as features correspondentes não estão ativas no grafo Cargo atual; ao habilitar
tray-icon ou linux-libxdo, revise essa lista. A cobertura e o setup dos instaladores
permanecem os mesmos. Os builds usam
`--locked` para preservar o Cargo.lock. O Stockfish usa uma chave que inclui o
script de download e é salvo logo após o download, antes dos testes/builds.
O sidecar é baixado antes dos testes Rust. Os testes do downloader também são
executados quando incluídos no comando `bun run test` do app.

O fluxo usa somente `GITHUB_TOKEN`, sem PAT ou GitHub App. Habilite em
**Settings → Actions → General → Workflow permissions** a opção
**Allow GitHub Actions to create and approve pull requests**. Os workflows concedem
permissões de escrita apenas aos jobs que precisam delas. O bot não aprova nem faz
merge dos seus próprios PRs.

Eventos criados pelo token podem não iniciar CI automaticamente. Por isso o fluxo
dispara `ci.yml` explicitamente na branch do PR, passando `pr_number`; o CI confere
se o SHA executado ainda corresponde ao PR. Para repetir manualmente:

```bash
gh workflow run ci.yml --ref NOME_DA_BRANCH -f pr_number=NUMERO_DO_PR
```

## Falhas e retomada

Falhas em testes ou em qualquer plataforma mantêm a release como rascunho.
A publicação exige os três instaladores completos. Todos são construídos no SHA
imutável da release; uma tag apontando para outro commit causa falha.

Use **Actions → Release → Run workflow**, selecione `main` e informe a tag, ou:

```bash
gh workflow run release.yml --ref main -f tag=v1.0.0
```

A retomada aceita apenas uma release existente, com tag coerente e commit no
histórico da main. Se já estiver publicada, somente retoma a sincronização com
develop. Não move tags nem cria uma nova versão.

## Verificar alterações na automação

Na raiz:

```bash
python -m unittest discover -s .github/scripts -p 'test_*.py'
python .github/scripts/release_version.py
```

Em `.github/scripts/`:

```bash
bun install --frozen-lockfile
bun run test
```

Execute também as verificações do app descritas no README. Ao atualizar a action
Release Please, mantenha a dependência dos testes na mesma versão da biblioteca
embutida na action. O CI valida a configuração usando essa implementação real.
