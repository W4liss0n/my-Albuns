---
status: accepted
document: design
date: 2026-09-17
platform: windows
implementation-readiness: ready-for-agent
---

# Ciclos de decisão e conclusão nativa

A terceira revisão de arquitetura identificou três consolidações, aprovadas em
conjunto. Elas complementam o [design 0037](0037-consolidacao-dos-proprietarios-internos.md)
sem alterar comandos, apresentação ou contratos persistentes.

## Decisões do Projeto

`createProjectDecisions` possui a vigência de uma decisão, a aceitação única de
ações, a apresentação e o encerramento. Conversão da extremidade da Lâmina,
remoção de mídias, exclusão de Layout e Informações do Álbum usam esse módulo.
Uma nova revisão pode pedir outra decisão na mesma sessão de diálogo. O resultado
normal só é entregue depois de `dismiss`; o descarte do consumidor resolve a
operação como cancelada e impede que trabalho tardio afete uma nova decisão.

Cada consumidor continua dono de seu contexto, da revalidação e do comando.
Conversão propaga falha de fechamento; os outros três fluxos conservam a política
de ignorá-la. `ProjectDialogPort` permanece dono da fila nativa e da posse da
janela. O módulo não duplica a fila nem o protocolo de `observeSnapshot`.

## Transição de identidade

`project_identity_transition` conclui a operação de Salvar como: pausa do Cache,
chamada ao Host/Core, reserva do namespace vazio, transição de WebView e título,
encerramento da Recuperação anterior, compensação e finalização. O comando IPC
recebe o resultado estabilizado; não recebe reservas ou recursos por finalizar.

O Core permanece dono da publicação e da classificação de falhas. O módulo
preserva a sessão e seu Histórico, a independência do original e a reserva de
WebView compartilhada com Recuperação, conforme os designs
[0015](0015-contrato-publico-de-persistencia-do-project-core.md) e
[0034](0034-recuperacao-da-interface.md). Se a restauração de título ou WebView
falhar, conserva a política de encerramento do processo. Uma falha na restauração
do título não impede a tentativa de restaurar o WebView.

Apenas a apresentação possui um adapter substituível nos testes headless. Eles
usam Host, Core, Cache e Recuperação reais, incluindo falhas reais de armazenamento,
e observam o resultado da operação completa.

## Conclusão de comandos criativos

`project_creative_commands` concentra captura dos vínculos anteriores, mutação,
preparação de imagens e releitura autoritativa da projeção. Aplicar intenção,
Desfazer e Refazer usam a mesma execução. `ProjectUiOperation` permanece viva até
a projeção final, inclusive durante preparação pendente; uma mutação rejeitada
não inicia preparação. Logs e resultados específicos continuam nos comandos IPC.

Core mantém Histórico; `prepare_changed_images` mantém a seleção de fontes novas
ou alteradas. N1 e R4 continuam donos da admissão e da confirmação de imagens.

## Verificação

- Decisão: ação duplicada, reconfirmação, falhas de aquisição/apresentação,
  espera pelo fechamento, descarte e conclusão tardia; testes dos quatro fluxos.
- Salvar como: sucesso com Histórico conservado e namespace vazio; falhas de
  destino, Cache, WebView, título e Recuperação; falhas de compensação; original
  preservado e reservas liberadas em todos os terminais exercitados.
- Comandos: recuperação espera a preparação; captura anterior à remoção e ao
  Histórico; releitura inclui observações de fontes publicadas durante a
  preparação; falhas liberam a operação.

## Contratos externos consultados

React instalado: 19.2.8. Tauri instalado: 2.11.5. A consulta ao Context7 estava
indisponível por cota. As referências oficiais consultadas foram
[limpeza de Effects](https://react.dev/reference/react/useEffect) e
[execução bloqueante do Tauri 2.11.5](https://docs.rs/tauri/2.11.5/tauri/async_runtime/fn.spawn_blocking.html).
O ciclo React mantém setup/cleanup simétricos. A transição bloqueante transfere
seus recursos ao worker e os conclui antes de devolver o resultado.
