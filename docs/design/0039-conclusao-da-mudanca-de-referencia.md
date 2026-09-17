---
status: accepted
document: design
date: 2026-09-17
platform: windows
implementation-readiness: ready-for-agent
---

# Conclusão da mudança de referência

A quarta revisão de arquitetura identificou a conclusão repetida da Religação
e da Substituição de imagem no Painel e na Exportação normal. A consolidação
foi aprovada sem alteração dos comportamentos descritos no
[Painel de imagens](0031-painel-de-imagens.md) e na
[Tela de Problemas](0005-tela-de-problemas.md).

`project_media_reference` conclui a mudança individual da referência da Sessão:
pausa do Cache, reserva de inspeção, proposta de `MediaResolver`, rejeição de
vínculo duplicado, invalidação, comando do Host, preparação do vínculo confirmado
no mesmo plano e releitura da projeção final. A pausa e a reserva de inspeção
terminam antes da preparação; não existe atividade de Cache aninhada.

Os dois consumidores usam essa conclusão. Seletores, busca, captura do contexto
de caminhos, conjunto de imagens da Exportação, notas e logs permanecem nos
adapters. A operação de interface continua abrangendo a seleção e a conclusão
inteira. Religação temporária do lote conserva seu proprietário e seu contrato.

O Core permanece dono do Histórico. O módulo reutiliza `MediaResolver`,
`CacheEngine` e `ImageProcessingBatch`; não duplica suas políticas. Falha de
preparação é informada pelo progresso e não desfaz um vínculo válido já aplicado.
A Exportação preserva sua reinspeção do conjunto e o diagnóstico de processamento.

## Verificação

A interface de conclusão é exercitada com Host, Core, Cache, inspeção e arquivos
reais. Apenas a preparação externa é controlada para observar espera, resultado
final e falha após o commit. Os testes preservam Undo/Redo, Salvamento manual,
isolamento da ocorrência, rejeição de Original inválido ou referência duplicada,
propagação do plano e liberação das reservas antes da preparação.

Os testes públicos existentes do Painel, Exportação, Host e invalidação do Cache
continuam necessários; a extração não substitui as suas provas de domínio.

## Contrato externo

Tauri instalado: 2.11.5. O Context7 estava sem cota; foram consultadas as páginas
oficiais de [spawn_blocking](https://docs.rs/tauri/latest/tauri/async_runtime/fn.spawn_blocking.html)
e [block_on](https://docs.rs/tauri/latest/tauri/async_runtime/fn.block_on.html),
identificadas nessa versão, e o código local do pacote. O worker possui o
AppHandle e termina a inspeção bloqueante; a conclusão aguarda seu resultado e
a preparação antes de devolver a projeção.
