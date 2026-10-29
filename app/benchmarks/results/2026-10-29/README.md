# Pipeline Rust: comparação de 29/10/2026

As otimizações reutilizam os buffers de PV durante o parsing UCI e emprestam as
avaliações já mantidas pelo modo automático para seleção e montagem final. Não
mudam perfis, budgets, quotas, prioridades, número de workers ou regras de cache.

## Método

Executáveis release preservados, mesmo Rust, Stockfish, corpus e perfis. A série
sem instrumentação alterna baseline/candidato/candidato/baseline, com três
lotes de aquecimento e 15 amostras por processo: 30 amostras por versão/cenário.
Cada amostra contém 300 buscas ou cinco revisões; tempos são por operação.
As medições instrumentadas são separadas e seus tempos não sustentam ganhos.
Os resultados completos e contadores fake devem ser iguais entre versões.

Na série principal (`separated-abba`) não havia compilação concorrente. Nas
séries anteriores de latência e Stockfish, o compilador foi suspenso durante a coleta.
O ambiente está em [environment.json](environment.json).
Este é um computador de uso geral; as distribuições e a alternância reduzem,
mas não eliminam, efeitos de temperatura, frequência e outras cargas do sistema.
Quartis não são intervalos de confiança. Não foram medidos IPC ou renderização.

## Tempo da pipeline com engine simulada

| Cenário | Baseline (ms) | Candidato (ms) | Variação da mediana |
| --- | ---: | ---: | ---: |
| Busca UCI, três linhas | 0,09651 | 0,08943 | -7,3% |
| Manual, cache frio | 13,522 | 13,798 | +2,0% — inconclusivo |
| Manual, cache quente | 1,411 | 1,377 | -2,4% — inconclusivo |
| Rápido, cache frio | 23,772 | 21,997 | -7,5% |
| Rápido, cache quente | 4,982 | 4,713 | -5,4% |
| Profundo, cache frio | 23,822 | 22,207 | -6,8% |
| Profundo, cache quente | 4,957 | 4,651 | -6,2% |

Os tempos manuais são inconclusivos: houve sobreposição e deriva temporal no
manual frio. A busca isolada e o manual frio
exercitam o parser; o automático quente não faz buscas e isola a remoção das
cópias na seleção/montagem. O fake usa o contrato de engine injetado e não
representa os tempos, concorrência ou memória do pool Stockfish real. Seu
repositório também é simulado: não inclui SQLite.

[Resumo principal com distribuições e memória](separated-abba/summary.json). Os JSONs completos de cada
processo estão no mesmo diretório, comprimidos com gzip sem alterar o conteúdo.

## Stockfish real, profundidade fixa

A/B/B/A, uma amostra de aquecimento e três amostras por processo, seis por versão.
Partida `real1`, profundidade 8, uma thread e 16 MB de Hash. Inclui processo,
pipes e teardown com adaptador headless, repositório fake; exclui o shell Tauri,
SQLite, frontend IPC e renderização.

| Linhas | Baseline (ms) | Candidato (ms) | Interpretação |
| --- | ---: | ---: | --- |
| 1 | 745,5 | 700,2 | Diferença de 6,1%, com grande variação e sobreposição; inconclusivo |
| 3 | 1208,1 | 1205,6 | Diferença de 0,2%; inconclusivo |

Os resultados completos observados e os contadores foram iguais. A série não
sustenta uma aceleração geral da análise real. A redução do overhead Rust não
altera a quantidade de trabalho de busca do Stockfish.

[Resumo com distribuições](real-latency-abba/summary.json).

## Diagnóstico automático com shell e SQLite reais

[Baseline](quality-baseline.json) e [candidato](quality-candidate.json): três
sequências de cinco lances com FEN, dois perfis, cache frio, total de seis threads
/ 96 MB de Hash e referência de 5 s por posição. Uma amostra por caso.

Não houve divergência de classificação contra a referência em nenhum dos seis
casos de cada versão. No candidato, troca/Rápido teve erro médio de avaliação de
1,45 ponto percentual contra sua referência, versus zero no baseline; os demais
casos tiveram zero. Buscas por tempo com múltiplas threads variam, e este teste
não permite atribuir a diferença a uma causa nem estabelecer equivalência geral
de força. A referência também não é uma verdade absoluta.

As durações permaneceram em aproximadamente 5,3–5,5 s no Rápido e 14,5–14,6 s no
Profundo. Uma amostra não sustenta ganho de velocidade. Estes dados não justificam
alterar quotas, budgets ou scheduling nesta mudança.

## Revisão e correção

Três rodadas de revisão com GPT-6.1 Terra. A primeira encontrou uma corrida no
reset do pico do contador de memória; a correção sincroniza início/fim e todas
as atualizações sob o mesmo mutex. O teste concorrente passou e a segunda rodada
aprovou a correção; a terceira auditou os dados, hashes e reprodução, sem novos
achados. Os dados de memória anteriores a essa
correção não integram este relatório.

Os testes cobrem equivalência de revisões/alvos com referências, crescimento e
redução de PV, profundidades iguais/regressivas, mate, bounds, MultiPV, falhas,
cache, cancelamento e encerramento. A suíte Rust também executou a sessão real
com Stockfish, cache, histórico, reabertura e cancelamento ao vivo.

## Memória e CPU

O contador instrumentado usa o mesmo mutex nas fronteiras da amostra e nas
atualizações. Mede chamadas de alocação/realloc, bytes solicitados acumulados e
pico incremental de bytes solicitados ainda vivos acima do início da amostra.
Não mede overhead do alocador, fragmentação, stack, mapeamentos nem filhos.
O pico é por lote de cinco revisões (300 buscas no cenário isolado); tráfego e
chamadas são normalizados por operação. Não use o tempo instrumentado como ganho.

| Cenário | Bytes alocados por operação, antes → depois | Redução | Redução de chamadas | Redução do pico incremental |
| --- | ---: | ---: | ---: | ---: |
| Busca UCI | 21.778 → 12.478 | 42,7% | 37,8% | 0,2% |
| Manual frio | 3.350.930 → 2.148.130 | 35,9% | 33,8% | 0% |
| Manual quente | 548.799 → 548.799 | 0% | 0% | 0% |
| Automático frio, ambos os perfis | 7.182.788 → 5.111.543 | 28,8% | 33,5% | 8,3% |
| Automático quente, ambos os perfis | 2.061.266 → 1.896.521 | 8,0% | 15,2% | 7,9% |

O pico do automático frio caiu de 701.040 para 642.639 bytes; no quente, de
735.721 para 677.320 bytes. A diferença de 58.401 bytes é do pico incremental do
lote no harness, não do RSS total do app. O RSS Rust variou em torno de 20–29 MiB
sem redução consistente. A memória dos processos Stockfish, incluindo Hash,
não foi incluída e seu orçamento permanece igual.

CPU de usuário/sistema é contabilizada por processo inteiro via `wait4`, incluindo
preparação, aquecimento e serialização. Com Stockfish inclui os filhos aguardados;
não separe esses valores como se fossem somente CPU Rust. As distribuições brutas
permitem conferir CPU sem inferi-la apenas do tempo de parede.

## Séries adicionais e limites

A primeira [série de latência](latency-abba/summary.json) observou redução de
5,4–10,8% no automático, 7,6% nas buscas e 6,1% no manual frio. Uma série
[intercalada com instrumentação](exploratory-interleaved-abba/summary.json)
mostrou maior deriva no manual frio. Por isso, a série principal termina a fase
de latência A/B/B/A antes da fase de alocações A/B/B/A de cada cenário.
O manual frio variou de redução de 17,5% a aumento de 2,0% entre séries; não há
evidência robusta para quantificar seu ganho de tempo. Todos os resultados foram
preservados, incluindo os desfavoráveis.

O ganho de pico é limitado ao contador Rust do harness. Não foi demonstrada
redução de RSS nem aceleração geral com Stockfish real. As sequências de qualidade
são pequenas, as buscas por tempo variam e não calibram a força do modo automático.

Na série principal, CPU de usuário + sistema por processo caiu 7,1% no Rápido
frio, 6,6% no Profundo frio e aproximadamente 5,2–5,4% nos automáticos quentes.
São medianas de dois processos por versão, com o mesmo trabalho; não são
contadores de hardware ou uma estimativa de CPU apenas da busca.

## Reprodução

Use um checkout dedicado do commit desta mudança, com dependências e sidecar
instalados. [sources.json](sources.json) fixa os hashes das fontes e executáveis;
a base pública `933532e` tem as mesmas fontes de produção do baseline medido.

1. Em `app/`, substitua somente as fontes otimizadas e seus testes pelo baseline:

   ```bash
   git restore --source=933532e -- src-tauri/src/review/adaptive.rs src-tauri/src/review/automatic.rs src-tauri/src/review/core/build.rs src-tauri/src/review/engine.rs src-tauri/src/review/tests/core.rs
   ```

2. Ainda em `app/`, compile e preserve as duas variantes com o harness atual:

   ```bash
   python3 - <<'PY'
   import sys, shutil
   sys.path.insert(0, 'benchmarks')
   from run import binary
   for instrumented, name in [(False, 'normal'), (True, 'alloc')]:
       shutil.copy2(binary(instrumented), f'/tmp/baseline-{name}')
   PY
   ```

3. Restaure essas cinco fontes ao commit atual com `git restore --` seguido dos
   mesmos caminhos. Repita o bloco Python usando `/tmp/candidate-{name}`.
4. Sem builds ou outras cargas pesadas concorrentes, em `app/`:

   ```bash
   python3 benchmarks/experiment.py --baseline-normal /tmp/baseline-normal --baseline-alloc /tmp/baseline-alloc --candidate-normal /tmp/candidate-normal --candidate-alloc /tmp/candidate-alloc --output /tmp/comparison
   ```

O benchmark real usa `--engine stockfish --scenario stockfish-real1-depth-pv1`
e/ou `--scenario stockfish-real1-depth-pv3`. Configure `BENCH_STOCKFISH` com o
caminho absoluto do mesmo binário nas duas versões. Para repetir o diagnóstico
automático, execute o teste ignorado `benchmark_stockfish_review_quality` com
cada executável normal e `BENCH_REPORT_PATH`; o sidecar `stockfish` deve estar
no diretório esperado pelo shell, ao lado do executável preservado.

## Verificação observada

- `bun run lint && bun run typecheck && bun run test`: 3 testes do downloader e
  135 testes Vitest passaram no worktree isolado.
- `cargo fmt --check`, `cargo test`: 120 testes unitários e o handshake passaram,
  incluindo a sessão real exigida e o pool real. O harness completo e o diagnóstico
  de qualidade ignorados na suíte normal foram executados explicitamente; o
  benchmark resumido antigo não foi usado.
- `cargo test --features bench-alloc sample_boundaries_and_concurrent_allocations_preserve_peak`:
  teste concorrente do contador passou.
- `cargo clippy --all-targets --all-features -- -D warnings`: passou.
- Após atualizar a branch sobre `develop`, os hashes das fontes candidatas
  continuaram idênticos aos executáveis medidos e a suíte Rust/clippy passou novamente.
