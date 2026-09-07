---
status: current
document: research
date: 2026-09-06
ticket: 19
platform: windows
---

# Fechamento da importação e das miniaturas

O ciclo de melhorias da importação múltipla de Fotos JPEG e das miniaturas está
concluído localmente. O usuário confirmou a correção final de abertura e
rolagem. A implementação está na branch `codex/import-multiple-jpeg`, com
`13820b855104fa5864028b568eee580356ec3ca8` como commit do código entregue.
A branch está publicada na [PR #65](https://github.com/W4liss0n/my-Albuns/pull/65),
com base na [PR #64](https://github.com/W4liss0n/my-Albuns/pull/64).
Este registro consolida a entrega e suas evidências; os contratos de produto
continuam nas fontes normativas abaixo.

## Resultado entregue

- Uma seleção importa vários JPEGs. Arquivos válidos entram mesmo quando
  outros são rejeitados; duplicatas não criam vínculos adicionais. Os novos
  vínculos formam uma única ação de Histórico, preservando os Originais.
- A validação e a geração da prévia aproveitam o mesmo decode. O processamento
  usa lotes e capacidade compartilhada, limitada pelos recursos disponíveis.
- O progresso avança por imagem durante o trabalho. Recuperações não contam
  a mesma imagem duas vezes; a conclusão aguarda a publicação e o preparo das
  miniaturas visíveis.
- Voltar a fotos já visitadas reutiliza as prévias ainda residentes, com
  limites explícitos de memória e quantidade.
- Ao abrir o Projeto, o Painel solicita suas primeiras imagens sem precisar
  de rolagem. A recuperação aproveita o Cache validado e a pré-carga antecipa
  as próximas linhas.

O recorte de importação permanece em `Fotos → Importar → Arquivos JPEG…`.
Importação por pasta, outros formatos e ampliação da importação de Decorativos
continuam fora deste fechamento; ele não encerra todo o escopo do ticket #19.

## Organização da implementação

| Responsabilidade | Proprietário |
| --- | --- |
| Tentativa de importação e progresso dos lotes | [photo_import.rs](../../src-tauri/src/photo_import.rs) e [photo_import/progress.rs](../../src-tauri/src/photo_import/progress.rs) |
| Decode, preparação e capacidade de processamento | [myalbuns-imaging](../../crates/myalbuns-imaging/src/photo_import.rs), [image_processing.rs](../../src-tauri/src/image_processing.rs) e [imaging_processor/resources.rs](../../src-tauri/src/imaging_processor/resources.rs) |
| Índice, gerações, publicação e recuperação do Cache | [cache_engine.rs](../../src-tauri/src/cache_engine.rs), com módulos internos de [importação](../../src-tauri/src/cache_engine/import.rs), [índice](../../src-tauri/src/cache_engine/index.rs) e [publicação](../../src-tauri/src/cache_engine/publication.rs) |
| Residência, limites e URLs das prévias | [cache_previews.rs](../../src-tauri/src/cache_previews.rs) |
| Observação das origens e adoção da evidência validada | [media_runtime.rs](../../src-tauri/src/media_runtime.rs), conectado à abertura em [product_runtime.rs](../../src-tauri/src/product_runtime.rs) |
| Área visível, pré-carga e interação do Painel | [MediaPanel.tsx](../../src/components/MediaPanel.tsx) e [mediaPanelViewport.ts](../../src/components/mediaPanelViewport.ts) |

O núcleo continua sendo o dono do estado criativo. O Cache é descartável e
não entra no Histórico; a Exportação continua usando os Originais. Os testes
acompanham os módulos e incluem o [percurso nativo da importação](../../src-tauri/src/photo_import/native_flow_tests.rs).

## Contratos e histórico

As regras vigentes estão em [Importação com decode único e lotes](../design/0020-importacao-com-decode-unico-e-lotes.md),
[Armazenamento local e Cache](../design/0010-armazenamento-local-e-cache.md),
[Progresso de operações](../design/0007-progresso-de-operacoes.md) e no
[ADR da arquitetura](../adr/0005-adotar-tauri-react-rust.md).

O histórico de commits foi preservado. Estes marcos orientam a leitura da
implementação final e das correções que a completaram:

| Marco | Resultado e evidência |
| --- | --- |
| `80b50b8` | Paralelismo com limite por Projeto, posteriormente integrado ao orçamento compartilhado. |
| `f582efd` / `d4c737b` | Otimização do Cache e entrega conjunta das miniaturas visíveis ao concluir a importação. |
| `35378c0` | [Reuso da inspeção e das gerações verificadas](2026-09-06-reuso-da-inspecao-e-do-cache-na-importacao.md). |
| `ec47139` | [Integração do decode único, lotes e orçamento de recursos](2026-09-06-integracao-importacao-producao.md). |
| `ffa44c8` | [Progresso enquanto o processamento está ativo](2026-09-06-progresso-importacao.md). |
| `3a0d072` | [Retenção das prévias recentes durante a rolagem](2026-09-06-retencao-previas-painel.md). |
| `13820b8` | [Carregamento inicial e primeira passagem pelas fotos](2026-09-06-abertura-e-primeira-rolagem-do-painel.md). |

Os relatórios registram o contexto de cada medição. Menções anteriores a
confirmação manual pendente ou a executáveis intermediários devem ser lidas
como estado daquela etapa; o executável de referência deste fechamento está
identificado a seguir.

## Validação realizada

A integração passou pela validação completa do repositório. As correções
posteriores tiveram testes de regressão e novas verificações dos módulos
afetados; os números e comandos de cada rodada estão nos relatórios acima.

Na correção final passaram 374 testes nativos do Host, com 15 casos dependentes
de ambiente ignorados, e 99 testes de interface/adaptador. Os sete casos
selecionados de reabertura passaram novamente após o ajuste final. Contratos,
TypeScript, formatação, Clippy e a compilação otimizada também passaram.

O percurso na aplicação nativa usou um Projeto isolado com 134 Fotos: o Painel
carregou sem movimentação inicial, todas as fotos foram vistas durante a
rolagem e o retorno conservou as miniaturas. O salto de 600 pixels completou
33 imagens visíveis em aproximadamente 255 ms; a passagem corrigida não
iniciou outro Processador de Imagens. A medição foi feita em Debug, com as
portas nativas reais; não mede a pintura física da GPU nem estabelece latência
zero para saltos além da margem de pré-carga.

Os cinco cenários visuais afetados — Fotos, seleção de Decorativo, popup por
teclado, importação em andamento e menu de importação — foram capturados e
revisados no commit `13820b8`: cinco aprovados, nenhum rejeitado ou sem validação.
Os testes preservaram o arquivo do usuário e os Originais.

## Executável e evidências locais

O build de referência usa o perfil **Release** e o commit de código `13820b8`.
Este fechamento documental é posterior ao build e não altera seu código.
Para testar, feche a versão anterior e execute `myalbuns-desktop.exe`, mantendo
`myalbuns-imaging.exe` na mesma pasta.

- [Executável do aplicativo](D:/CodexBuilds/myalbuns-initial-previews/release/myalbuns-desktop.exe).
- [Manifesto com commit, perfil, tamanhos e hashes dos dois executáveis](D:/CodexBuilds/myalbuns-initial-previews/release-manifest.json).
- [Revisão visual](D:/CodexBuilds/myalbuns-initial-previews/ui/review-report.html) e [evidências das capturas](D:/CodexBuilds/myalbuns-initial-previews/ui/evidence.json).
- [Medição da abertura e do salto de rolagem](D:/CodexBuilds/myalbuns-initial-previews/fixed-measure.json) e [percurso com eventos de roda](D:/CodexBuilds/myalbuns-initial-previews/fixed-gradual-measure.json).

Esses artefatos pertencem à máquina de validação e ficam fora do Git. Os
relatórios versionados preservam o resultado e os limites da evidência.
Pastas e executáveis intermediários não substituem o build identificado aqui.

## Situação da entrega

A implementação, os testes locais e a confirmação do usuário estão concluídos
para este recorte. O README e este registro consolidam o fechamento documental.
A branch e a PR #65 estão publicadas. A base é
`codex/interaction-and-native-gates-followups`, da PR #64, cuja aceitação nativa
de fechamento ainda está pendente. A publicação não conclui esse aceite nem a
integração em `main`; a issue #19 continua aberta para o restante de seu escopo.

A revisão final do intervalo `ec47139..8262c93` não encontrou achados pendentes
nos eixos de aderência aos padrões e à especificação. A validação completa
adicional em `8262c93` identificou uma expectativa antiga no teste de
redimensionamento do Painel: conservar os observadores mesmo quando o tamanho
dos cards mudava. O contrato atual requer recalcular a geometria. O teste foi
atualizado para conferir a demanda antes e depois do redimensionamento, a
desconexão dos observadores antigos, a rejeição de callbacks obsoletos e a
rolagem posterior. Abrir somente o popup continua sem reiniciar os
observadores. Esse ajuste modifica apenas o teste; o código do executável de
referência permanece `13820b8`.
