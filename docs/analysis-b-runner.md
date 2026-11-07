# Executor experimental B

O exemplo standalone executa um worker sem acessar SQLite. Em `app/src-tauri/`:

```sh
cargo run --example analysis_b -- examples/fixtures/analysis-b-mate.pgn binaries/stockfish-x86_64-unknown-linux-gnu examples/fixtures/analysis-b-manifest.json /tmp/engineroom-analysis-b.json
sha256sum binaries/stockfish-x86_64-unknown-linux-gnu
```

O manifesto JSON versão 1 fornece profundidades de screening e verificação, limiar em centipawns, máximo de tentativas, MultiPV, Threads, Hash em MiB, timeout por operação em milissegundos e cancelamento opcional em milissegundos. O campo de cancelamento pode ser omitido ou `null`; os demais são obrigatórios; campos desconhecidos e valores fora dos limites são rejeitados. A verificação deve ter profundidade maior que o screening. As opções efetivas solicitadas são exportadas em ordem canônica; o manifesto não altera `profiles.json`. A política está em `analysis-b-contract.md`.

O relatório versão 2 inclui PGN, caminho canônico, manifesto efetivo, SHA-256 do arquivo iniciado, opções, snapshot, status e erro estruturado com código, operação e mensagem. `protocol_completed` indica término sem pendências; `unresolved` indica término com pendências ou conclusão incompleta; `cancelled` identifica cancelamento; `failed` identifica falha técnica. Cancelamento e falha retornam código de processo não zero depois de gravar o relatório. Erros anteriores à criação do planner ou de escrita não produzem relatório.

`snapshot.metrics` registra parede desde o início da aquisição até o fim do flush, contagens de cache e buscas, soma da parede das buscas (inclui sobreposição entre workers) e tempo entre observar cancelamento e terminar o shutdown dos processos. Buscas iniciadas sem evidência aceita contam como descartadas. No Linux, os PIDs dos processos são amostrados em `/proc` a cada 50 ms e antes do shutdown: `cpu_ms` soma CPU acumulada até a última amostra válida, incluindo a inicialização e excluindo o trabalho após essa amostra; `peak_sampled_rss_bytes` é o maior RSS agregado *amostrado*, não necessariamente o pico real. Uma amostra CPU incompleta, PID indisponível, plataforma não suportada ou task com panic deixa a respectiva métrica indisponível (`null`); não se estima CPU pelo orçamento ou por `movetime`. O ledger por busca continua sem atribuição de CPU ou memória. O tempo de cancelamento começa quando o executor observa o sinal, não quando o usuário o acionou.

O caminho deve apontar diretamente para o executável da engine: scripts são recusados e o arquivo deve ser regular, com permissão de execução em Unix. O formato é restrito a ELF no Linux, PE no Windows e Mach-O no macOS; outras plataformas são recusadas. Essa verificação de cabeçalho não prova que um executável nativo não seja um wrapper, nem autentica Stockfish; não use wrappers. O digest identifica apenas os bytes do arquivo informado, não bibliotecas carregadas ou qualquer outro processo que ele possa iniciar. Também não impede substituição concorrente do arquivo entre leitura e spawn. Use um binário confiável e imutável para reprodução. O protocolo UCI é verificado na inicialização.

O driver envia raiz e prefixo, aceita conjuntos completos de PVs legais e exatas, e conserva o último conjunto completo quando chega uma atualização parcial. O runner encerra e aguarda o processo antes de retornar, inclusive em timeout e cancelamento. Este executor experimental não adquire o lease do aplicativo e não deve ser executado concorrentemente com ele quando exclusividade for necessária.

Para verificar cleanup durante buscas reais no Linux, execute `cargo build --example analysis_b` e depois `python examples/fixtures/test_analysis_b_ownership.py`. O teste depende do sidecar Linux indicado no comando acima e de procfs.

O caminho de saída deve ser novo: a CLI usa criação exclusiva e recusa qualquer arquivo existente, incluindo aliases de entradas, para evitar truncar PGN, manifesto ou engine. Remova ou escolha outro relatório para repetir uma execução.
