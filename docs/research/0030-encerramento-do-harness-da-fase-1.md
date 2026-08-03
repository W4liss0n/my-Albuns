---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-08-02
updated: 2026-08-02
---

# Encerramento do harness da Fase 1

## Decisão

Os runners, probes e corpus usados para comparar as topologias A e B eram
instrumentos temporários de pesquisa. Depois da escolha da topologia A — um
host independente por Projeto —, eles deixaram de ser uma parte executável do
produto. O harness completo continua recuperável no commit `b40c782` e seus
resultados permanecem nos artefatos e relatórios da Fase 1.

Manter esses instrumentos no runtime, nos scripts correntes e nos comandos do
`package.json` preservaria a topologia B em paralelo e acrescentaria caminhos
de falha, dependências e manutenção sem valor para o produto escolhido. Por
isso, o harness foi arquivado por remoção, em vez de transformado em um segundo
framework permanente de testes.

## Resíduos de implementação retirados

A auditoria posterior removeu também três implementações que haviam sobrevivido
apenas como código executável ou `#[cfg(test)]`:

- o `ProjectOpeningGuard`, que mantinha várias sessões editáveis no mesmo
  processo e portanto representava a topologia B descartada;
- o `BatchRunner` do host de Projeto, que antecipava o lote no composition root
  errado e não possuía consumidor de produção;
- o candidato `GlobalShell`, seu bundle, capabilities e diálogo, porque nenhuma
  janela global era criada pelo runtime atual.

As responsabilidades continuam normativas no ADR, mas serão implementadas nos
composition roots reais de seus tickets. A Tela de Boas-vindas futura identifica
e lança um host independente; ela não possui uma `ProjectSession`. O lote futuro
pertence ao processo global e reutiliza `ProjectCore`, `ExportPipeline` e
`OperationLease`, sem conservar uma implementação paralela dentro do host.

## Verificações que continuam correntes

As regressões úteis permanecem cobertas por testes alinhados à arquitetura
adotada:

- `npm test` cobre o comportamento da interface e seus adaptadores;
- `npm run build` valida contratos gerados, tipos e a build do frontend;
- `npm run test:rust` e `npm run quality:rust` cobrem o núcleo, integrações,
  formatação e lints Rust;
- `npm run contract:check` confirma que os DTOs gerados continuam sincronizados
  sem alterar a árvore de trabalho;
- `npm run test:imaging-recovery` preserva a prova pertinente de recuperação do
  Processador de Imagens;
- `npm run test:windows-path` preserva os casos de caminhos Windows, Unicode,
  nomes longos e identidade de arquivos.

Os gates de comparação A/B, distribuição do spike, processo global candidato e
falhas sintéticas não possuem substituto corrente porque verificavam
alternativas ou scaffolding que não existem mais no produto. Suas evidências
continuam históricas; uma futura validação de instalação em máquina limpa deve
ser criada em torno do distribuidor real, não ressuscitar o spike.

## Relatórios históricos

Os comandos e nomes de scripts citados nos relatórios abaixo descrevem como a
evidência foi coletada. Eles não são instruções executáveis da versão atual:

- [comparação preliminar de topologias](0007-comparacao-preliminar-de-topologias.md);
- [cache com imagens reais](0008-cache-com-imagens-reais.md);
- [pan, zoom e exportação com imagens reais](0009-pan-zoom-e-exportacao-com-imagens-reais.md);
- [recuperação do Processador de Imagens](0010-recuperacao-do-processador-de-imagens.md);
- [isolamento e recuperação de processos](0015-isolamento-e-recuperacao-de-processos.md);
- [gate operacional e operation lease](0017-gate-operacional-e-operation-lease.md);
- [matriz terminal da Exportação normal](0019-matriz-terminal-da-exportacao-normal.md);
- [lote e operation lease](0020-lote-e-operation-lease.md);
- [operation gate e bloqueio de abertura](0021-operation-gate-e-bloqueio-de-abertura.md);
- [ProjectCore, sessões e revisões persistidas](0022-project-core-sessoes-e-revisoes-persistidas.md);
- [caminhos, identidade e planos distribuídos](0023-caminhos-identidade-e-planos-distribuidos.md);
- [capabilities, permissions e scopes do frontend](0024-capabilities-permissions-e-scopes-do-frontend.md);
- [candidato a processo global de boas-vindas](0025-candidato-processo-global-de-boas-vindas.md);
- [distribuição Windows e pendência de máquina limpa](0026-distribuicao-windows-e-pendencia-de-maquina-limpa.md);
- [protocolo da comparação final de topologias](0027-protocolo-da-comparacao-final-de-topologias.md);
- [comparação final de topologias](0028-comparacao-final-de-topologias.md).
