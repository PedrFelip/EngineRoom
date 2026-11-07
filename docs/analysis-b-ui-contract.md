# Contrato proposto de interface da análise B, versão 1

**Estado: implementado no backend experimental.** A execução usa profundidade 8 para cobertura, 12 para verificação, MultiPV configurado, um worker, uma tentativa e limite de 120 segundos por operação. O progresso publicado atualmente informa início e término da cobertura; a análise ao vivo posterior usa o pipeline existente.

`review_session_open` aceita `config.pipeline` opcional: `"legacy"` ou `"experimental-b"`; ausência equivale a `legacy`. O resultado e eventos legados permanecem inalterados. B é temporário: não grava histórico nem usa cache legado.

Eventos usam o envelope existente, com os campos do evento no mesmo objeto: `{ "sessionId": string, "sequence": number, "type": ... }`. `requestId` é omitido para eventos da revisão inicial e aparece nos eventos de análise ao vivo. Os eventos de revisão B são exatamente:

```json
{"sessionId":"example","sequence":1,"type":"bProgress","stage":"coverage","completed":0,"total":10}
{"sessionId":"example","sequence":2,"type":"bCompleted","result":{"version":1,"pipeline":"experimental-b","persistence":"temporary","status":"protocolCompleted","positions":[{"ply":0,"fen":"..."}],"decisions":[{"ply":1,"version":1,"status":"preliminary","best":{"kind":"cp","value":20},"played":null,"bestUci":"e2e4","lines":[{"pv":["e2e4"],"depth":8,"value":{"kind":"mate","moves":3}}],"reason":null}]}}
```

`stage` é `coverage` ou `verification`; `completed` e `total` são contagens de posições na cobertura e de tentativas de verificação, respectivamente. `status` de resultado é `protocolCompleted` ou `unresolved`. O status de decisão é `preliminary`, `verified` ou `unresolved`. Valores são exclusivamente `{"kind":"cp","value":number}`, `{"kind":"mate","moves":number}` ou `{"kind":"terminal","outcome":string}`. Campos anuláveis sempre aparecem. Posições têm ply começando em zero; decisões identificam o lance com ply começando em um. As linhas e valores são projetados das evidências citadas pela conclusão; não se infere classificação crítica nem verificação apenas da existência de revisão. Erros e cancelamento seguem o protocolo existente; navegação ao vivo continua separada.
