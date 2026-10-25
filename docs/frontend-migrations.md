# Migração do frontend para a 2.1.2

Versões estáveis consultadas em 25/10/2026: Effect 4.0.6, TypeScript 7.0.2
e Vitest 5.0.5. As versões instaladas estão em `app/bun.lock`.

## Effect 4

O [guia oficial de migração](https://github.com/Effect-TS/effect/blob/main/MIGRATION.md)
e o [guia de Schema](https://github.com/Effect-TS/effect/blob/main/migration/schema.md)
orientam as adaptações:

- Os serviços injetados usam `Context.Service`, conservando suas chaves e contratos.
- A validação síncrona dos Channels usa `Schema.decodeUnknownExit` e `Exit`;
  a fronteira IPC assíncrona usa `Schema.decodeUnknownEffect` e `Schema.Codec`.
- Os schemas usam `Literals`, arrays de membros em `Union`, `check`,
  `fromJsonString` e a nova assinatura de `Record`. Os defaults de histórico e
  cache usam `withDecodingDefaultType`. A normalização de erro de processo
  opcional e anulável usa `SchemaGetter`, mantendo `null` como ausência.
- As APIs de recuperação, extração de causas e conclusão síncrona de Deferred
  usam os nomes do v4. O aviso de sessão usa `forkIn` com o escopo proprietário;
  o runner do v4 não recebe mais um escopo em suas opções.
- Aquisição e liberação continuam em `acquireRelease`; fechar a sessão aguarda
  a confirmação do backend. Os testes existentes de cancelamento e navegação
  continuam exercitando os contratos reais. Um teste com `TestClock.layer()` de
  `effect/testing` também verifica que a interrupção aguarda o finalizador.

Os testes continuam usando Vitest e serviços de teste do próprio Effect.
`@effect/vitest` 4.0.6 declara peers Effect ^4.0.6 e Vitest >=5 <6, mas não é
necessário adicionar o adaptador para esta suíte.

## TypeScript 7

O [anúncio oficial](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)
descreve a substituição do compilador por uma implementação nativa. O projeto
usa a CLI `tsc`; Vite e Biome não dependem da antiga API programática do
compilador para executar o build e o lint.

O `baseUrl` removido foi retirado: o alias `@/*` já aponta explicitamente para
`./src/*` em `paths`. A supressão de deprecações do TypeScript 6 também foi
retirada. O modo strict e a checagem separada dos testes permanecem ativos.

## Vitest 5

O [guia oficial](https://main.vitest.dev/guide/migration/) exige Node >=22.12
e Vite >=6.4. O projeto usa Vite 8 e os workflows usam Node 22 atualizado.
O ambiente continua sendo `node`. O novo padrão `clearMocks: true` é aceito;
os testes não dependem de chamadas de mocks acumuladas entre casos.

A suíte executa o downloader com Node antes do Vitest. O typecheck dos testes
continua separado, pois a transformação de TypeScript no Vitest não substitui
`tsc`.
