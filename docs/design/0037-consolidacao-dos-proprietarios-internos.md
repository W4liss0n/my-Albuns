---
status: accepted
document: design
date: 2026-09-17
platform: windows
implementation-readiness: ready-for-agent
---

# Consolidação dos proprietários internos

A segunda revisão arquitetural de 17/09/2026 identificou dez consolidações,
aprovadas em conjunto. O objetivo é retirar protocolos repetidos dos consumidores
e preservar os contratos do produto, inclusive Histórico, cancelamento,
recuperação de imagens e apresentação dos controles.

| Item | Proprietário e responsabilidade | Verificação |
| --- | --- | --- |
| N1 | `ImageWorkAdmission` coordena atividade do Cache, capacidade do Processador e a checagem de cancelamento. `ImageWorkLease` mantém as duas reservas até a drenagem. | Espera por capacidade, pausa da Exportação, obsolescência e fluxos reais de importação. |
| N2 | `usePropertyDrafts` mantém uma propriedade ativa sobre `useFrameCompositionDraft`: estilo, ângulo e os dois contratos de Zoom. | Previews tardios, troca de seleção, sequência entre propriedades, Salvar/Desfazer e falha do predecessor. |
| N3 | `FrozenProjectRendering::into_export` é a única preparação produtiva da Exportação. O invólucro antigo `FrozenSheetRendering` foi removido. | Exportação de Lâminas, fontes exatas, placeholders e protocolo do Processador. |
| N4 | `HeadlessBrowserSession` adquire e encerra Vite, WebDriver e seus descendentes. Os quatro cenários mantêm suas próprias asserções. | Execuções headless e falha na aquisição sem terminar processos alheios. |
| R3 | `PersistentProjectSession::apply` prepara cada candidato uma vez e publica pelo mesmo ponto de Histórico. | Edições sem alteração, Redo, Layout travado, Clipboard, geometria e transformação dimensional. |
| R4 | `MediaConfirmation` confirma observações e invalida o Cache antes de devolver um resultado sem reservas abertas. | Observação comum, demanda atual e demanda substituída; exclusividade após o retorno. |
| R5 | `observeSnapshot` instala a assinatura antes da leitura inicial e controla precedência e encerramento. | Evento durante a leitura, registro tardio, falha e descarte durante a inicialização. |
| R6 | `src/contracts/generated` contém os valores gerados compartilhados com o Host. Exportação e geração deixam de copiar suas formas manualmente. | Comparação com a geração Rust, typecheck e limite entre aplicação e adaptadores Tauri. |
| R7 | A projeção imutável do Álbum e `compose_album` servem editor, previews e congelamento sem criar sessões artificiais. | Mesma composição entre Canvas e Exportação, previews sem Histórico e transformações dimensionais. |
| R8 | `local_store_io::write_atomically` publica também a lista de Projetos recentes. | Promoção, ordenação, substituição e propagação de falha de gravação. |

## Contratos preservados

A admissão de imagem não executa o trabalho nem decide sua política de erro.
Cache mantém obsolescência terminal; importação mantém interrupção por memória,
resultado parcial e quarentena; monitoramento pode aproveitar uma inspeção já
importada antes de reservar o Processador. Religar continua dentro da pausa
exclusiva existente, sem adquirir atividade aninhada. O decoder iniciado é
drenado antes de liberar a reserva, conforme os designs
[0020](0020-importacao-com-decode-unico-e-lotes.md) e
[0032](0032-revisao-da-admissao-por-memoria.md).

O Zoom individual continua enviando `transformPhoto` com delta arredondado a
quatro casas. O Zoom da seleção continua usando `setPhotoZoom` absoluto. Pan e
gestos diretos do Canvas permanecem em `usePhotoGestures`. Ao mudar de propriedade,
o rascunho anterior entra na fila antes do seguinte. Salvar e Desfazer escoam o
rascunho ativo pela mesma fila; uma falha pendente cancela seus sucessores.
O proprietário mantém as operações pendentes separadas do rascunho ativo: trocar
de propriedade não reabilita o Zoom individual antes de seu delta terminar.

O Histórico conserva a distinção existente entre comandos que ignoram um
candidato igual e comandos que registram a ação. Apenas Informações do Álbum,
depois da validação integral e da confirmação dimensional, pode publicar a
transformação global de Frames travados. Clipboard continua independente do
Histórico criativo.

Eventos de mídia são emitidos pelos adaptadores depois da confirmação estável.
A demanda atual pode adotar a nova época; uma demanda substituída não ganha
novamente autoridade de publicação. O resultado de confirmação não transfere
ao consumidor a obrigação de invalidar o Cache ou de liberar uma reserva.

## Contratos externos consultados

As versões instaladas nesta implementação são React 19.2.8, Tauri API 2.11.1,
Tokio 1.53.1 e Node 24.18.0. A consulta ao Context7 estava indisponível por cota;
as referências oficiais usadas para os contratos envolvidos foram:

- [React: limpeza de Effects e repetição em Strict Mode](https://react.dev/reference/react/useEffect).
- [Tauri 2: assinatura de eventos e função de liberação](https://v2.tauri.app/reference/javascript/api/namespaceevent/).
- [Tokio 1.53.1: fila do RwLock e preferência de escritores](https://docs.rs/tokio/1.53.1/tokio/sync/struct.RwLock.html).
- [Rust: ordem de destruição dos campos](https://doc.rust-lang.org/reference/destructors.html).
- [Node: eventos de criação/falha de processos e `windowsHide`](https://nodejs.org/api/child_process.html).
