---
status: current
document: research
date: 2026-09-06
platform: windows
---

# Integração das otimizações de importação

A implementação incorpora no programa principal o percurso RGB das prévias
JPEG, leitura bufferizada do índice e tratamento da corrida de limpeza. A
importação usa resultados tipados do protocolo 18: o raster decodificado no
Processador fornece as dimensões orientadas e a prévia. Os `MediaId` continuam
sendo atribuídos pelo Core, em uma ação de Histórico e na ordem da seleção.

O `ImagingProcessor` conserva o controle dos filhos, suas reservas e contenção.
Cada filho atende até 32 fotos em sequência. CPU, RAM e commit disponíveis
limitam a admissão; a política não aprende continuamente a partir dos tempos
dos lotes. Inspeções alternativas, Monitor e Religação usam o mesmo orçamento.

As três mudanças arquiteturais da reanálise foram incorporadas:

- O `CacheEngine` planeja consultas por lote, publica gerações em uma transação
  e conclui os trabalhos compartilhados somente depois do índice persistido.
- Host e Processador usam a mesma implementação das regras de validação da
  prévia, mantendo a verificação independente nas duas fronteiras.
- A tentativa transporta catálogo, raízes e evidências; o Monitor adota
  explicitamente as inspeções confirmadas da importação.

O contrato completo está em
[0020 — Importação com decode único e lotes](../design/0020-importacao-com-decode-unico-e-lotes.md).

## Revisão e correções verificadas

As revisões de especificação e Standards examinaram o diff completo e as
correções posteriores, sem achados acionáveis pendentes. Foram corrigidos e
testados os seguintes casos:

- Original alterado entre a proposta e o commit: somente essa foto é rejeitada;
  a vizinha válida conserva suas dimensões e uma única ação de Histórico.
- JPEG acima do teto de pixels do Cache: o Processador recusa antes de iniciar
  o decode, preservando a possibilidade de inspeção alternativa isolada.
- Inspeção do Monitor aguardando recursos: a pausa de Exportação cancela a
  espera, alcança exclusividade e permite retomar a inspeção depois.
- Escritor cuja saída não foi confirmada: candidatos permanecem protegidos e
  a limpeza aguarda a recuperação por uma nova instância do Host.

Também há testes de equivalência RGB/RGBA, limites e cancelamento de reservas,
publicação conjunta, índice modificado/corrompido, candidatos abandonados,
falhas parciais e recuperação. Os testes existentes de Salvar/Fechar pendentes,
Undo e apresentação conjunta dos cartões integram a suíte da interface.

## Validação completa

`npm run validate` passou em 6 de setembro de 2026, cobrindo preparação do
Processador, contratos TypeScript, compilação da interface, testes da interface,
automações, formatação, Clippy e suíte Rust. Foram 782 testes da interface,
119 testes de automação aprovados (três pulados) e 647 execuções de testes Rust
aprovadas (18 ignoradas conforme a seleção da suíte).

O teste de encerramento do Host foi atualizado para verificar todos os oito
slots disponíveis, incluindo término dos filhos e recuperação dos registros.
O ensaio real de importação e o de Exportação são chamados explicitamente pela
suíte. A validação não abriu janelas nativas visíveis; seu relatório local está
em `.tools/validation/report.json`.

## Medição com 134 fotos

Foram usadas as 134 fotos já autorizadas para os ensaios anteriores, reunidas
em uma seleção. Cada rodada cria Projeto e Cache novos. Builds e outras suítes
ficaram parados durante a medição. Perfil debug, protocolo 18, teto de oito
trabalhadores; cada rodada iniciou oito processos para os lotes.

| Rodada | Processamento nativo | Proposta e conferência no Host | Commit | Primeira demanda e entrega | Total | Pico de trabalhos ativos |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 7,464 s | 1,345 s | 0,268 s | 0,262 s | 9,376 s | 7 |
| 2 | 7,473 s | 1,338 s | 0,263 s | 0,270 s | 9,381 s | 7 |
| 3 | 4,582 s | 1,364 s | 0,261 s | 0,274 s | 6,521 s | 8 |

O total também inclui aproximadamente 24–27 ms de captura e planejamento da
tentativa. A mediana foi **9,376 s**. A variação entre rodadas não é atribuída a
uma causa isolada: a política responde aos recursos disponíveis e o cache de
arquivos do sistema participa da medição.

Em cada rodada, as 134 fotos foram importadas, as 134 prévias responderam com
sucesso e não houve rejeições ou decodificações de Originais pelo Host. Os hashes
das prévias foram idênticos nas três rodadas. Os hashes dos Originais antes e
depois coincidiram. Undo removeu toda a importação em uma ação; o Monitor não
introduziu nova atualização após adotar a inspeção; o índice não existia antes
do commit e o diretório de mídia terminou com exatamente 134 prévias.

SHA-256 do Processador medido:
`ba52371221894bbc69564f0b7a959ce681d51b997cfd32ee669af1f831d8529a`.
Os dados brutos locais ficam em
`D:/CodexBuilds/myalbuns-import-production-measurement/report.json`.
Para repetir, o teste aceita `MYALBUNS_TEST_IMAGING_PROCESSOR`,
`MYALBUNS_IMPORT_MEASUREMENT_INPUTS` (JSON com caminhos),
`MYALBUNS_IMPORT_MEASUREMENT_ROUNDS` e
`MYALBUNS_IMPORT_MEASUREMENT_OUTPUT`.

## Limites da evidência

O ensaio `photo_import::native_flow_tests::real_import_flow` executa os módulos
de produção e o Processador real até o commit, adoção no Monitor e atendimento
das URLs de prévia. Não inclui o seletor nativo, a renderização da WebView nem
o transporte Tauri do comando. A contenção, os registros dos escritores e a
recuperação têm verificações próprias na suíte.

Os tempos deste ensaio usam compilação debug e não representam uma medição de
latência da interface nem uma comparação com a versão anterior. O cálculo de
hashes das fontes antes das rodadas aquece o cache de arquivos do Windows.
Não há percentual de aceleração derivado desses números.
