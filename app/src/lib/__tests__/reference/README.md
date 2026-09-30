# Referência da análise anterior

Esta implementação TypeScript fica congelada para os testes de paridade e os
benchmarks. O aplicativo usa as sessões de análise implementadas em Rust; não
importe estes módulos no código de produção.

Ao encontrar uma regressão no Rust, corrija o Rust sem alterar a referência para
fazer os resultados coincidirem. Mudanças deliberadas nas regras da análise
precisam de novos casos e de uma atualização explícita das expectativas.
