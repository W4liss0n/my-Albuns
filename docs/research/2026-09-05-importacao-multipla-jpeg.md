---
status: current
document: research
date: 2026-09-05
updated: 2026-09-05
ticket: 19
platform: windows-11-x64
---

# Importação de várias Fotos JPEG

Este é o primeiro recorte do [ticket #19](https://github.com/W4liss0n/my-Albuns/issues/19), aprovado na [revisão do produto](2026-09-05-estado-do-produto-e-proxima-entrega.md). A implementação parte de `eb204b3e6c16299bcc44d13c93bbca9dc2c9b166`, na branch `codex/import-multiple-jpeg`. O ticket continua aberto.

## Comportamento entregue

Na aba Fotos, `Importar → Arquivos JPEG…` permite escolher vários Originais na mesma seleção. Cancelar não altera o Projeto. Durante a seleção e a inspeção, o botão impede outra seleção simultânea. Após confirmar a seleção, um diálogo pertencente mostra a contagem real de arquivos analisados e a porcentagem geral. A importação não oferece cancelamento depois dessa confirmação. A fila existente ordena as alterações seguintes; a Exportação aguarda a conclusão da importação.

Os JPEGs válidos entram no Painel mesmo quando outros arquivos são rejeitados. Repetições na seleção e caminhos já presentes na aba não criam vínculos adicionais. Uma Foto já vinculada pode ser selecionada mesmo com o Original indisponível; reimportar não se transforma em Religação.

Os novos vínculos entram juntos em uma única ação do Histórico. Undo remove somente esses vínculos, Redo os restaura e as duplicatas preexistentes permanecem. Uma seleção sem novos vínculos não cria revisão, estado sujo nem Histórico.

Ao concluir com rejeições, o diálogo de Problemas mostra `Arquivo` e `Motivo`, com rolagem para listas extensas. Fechar ou pressionar Esc dispensa o resultado sem desfazer a importação. Se um Salvamento enfileirado falhar depois da importação parcial, dispensar esse erro preserva e mostra a lista de arquivos rejeitados. O progresso fecha antes do resultado. Sucesso integral abre uma confirmação curta em outro diálogo, sem acrescentar texto ao lado de Importar. Repetir somente Fotos já vinculadas informa isso na mesma confirmação. O último item aceito ou reencontrado passa a ser a seleção do Painel.

Salvar e reabrir preservam os vínculos externos. Os Originais não são regravados. Fotos importadas pelo lote seguem a composição existente e podem ser exportadas pelo Processador.

## Fronteiras usadas

- O comando nativo mantém os caminhos fora da WebView e usa `pick_files`. A inspeção ocorre no executor bloqueante do Tauri, fora do fluxo da interface.
- O MediaResolver captura as raízes em um único `OperationPathContext` por seleção, congela o plano e relata falhas por arquivo. O formato é conferido pelos bytes e os pixels JPEG são decodificados para rejeitar corrupção.
- O Host verifica a Identidade do Projeto antes de confirmar o resultado e agenda Recuperação somente quando há novos vínculos.
- O Core deduplica e confirma o documento inteiro uma única vez. Metadados de fonte continuam transitórios.
- Os contratos IPC são gerados por ts-rs. A Tela de Problemas usa o mecanismo de diálogo pertencente já existente; o componente tabular fica em `src/ui/`.

## Documentação externa consultada

Versões resolvidas: Tauri 2.11.5, `tauri-plugin-dialog` 2.7.2 e `image` 0.25.10. Nenhuma dependência foi adicionada.

O Context7 foi consultado para o plugin oficial de diálogos, a biblioteca image e o Channel do Tauri 2. O progresso usa um Channel por tentativa, configurado antes de invocar o comando e ignorado após a conclusão; não usa um serviço global de progresso. Referência: [Channels do Tauri 2](https://v2.tauri.app/develop/calling-frontend/#channels). A assinatura e o cancelamento de `pick_files` foram confirmados também na documentação de código da versão 2.7.2 instalada: callback `Option<Vec<FilePath>>`, com `None` ao fechar o seletor. A decodificação integral segue `DynamicImage::from_decoder`, também já usado no Cache do projeto. Referências: [FileDialogBuilder 2.7.2](https://docs.rs/tauri-plugin-dialog/2.7.2/tauri_plugin_dialog/struct.FileDialogBuilder.html#method.pick_files), [DynamicImage 0.25.10](https://docs.rs/image/0.25.10/image/enum.DynamicImage.html#method.from_decoder).

## Verificação e limites

As provas sem janelas estão nas fronteiras públicas: Core com arquivos reais e reabertura; Host/MediaResolver com JPEGs reais, falhas e duplicatas; fila da interface com comandos adjacentes em sucesso, cancelamento e falha; Painel acionando a porta de Problemas; contrato e fechamento acessível do diálogo. A prova existente de reabertura e Exportação com Processador real agora compõe uma Foto obtida numa importação de dois arquivos.

O comando de validação completa é `npm run validate`, com relatório local em [report.json](../../.tools/validation/report.json). A evidência visual inicial fica em [report.html](../../.scratch/ui-acceptance/import-multiple-jpeg-final/report.html), com decisões no [relatório revisado](../../.scratch/ui-acceptance/import-multiple-jpeg-final/review-report.html). A correção do progresso e da confirmação tem evidência em [import-progress/report.html](../../.scratch/ui-acceptance/import-progress/report.html), com decisões em [review-report.html](../../.scratch/ui-acceptance/import-progress/review-report.html). Capturas de componentes e testes de portas não são aceitação do seletor do Windows.

**Aceitação nativa pendente:** selecionar vários JPEGs e cancelar no seletor real; verificar propriedade, foco e fechamento do novo resultado na janela real. Essa rodada precisa ser coordenada separadamente. Nenhuma janela do aplicativo ou Sandbox foi aberta nesta implementação.

Este recorte não implementa PNG/TIFF, pastas, soltura do sistema operacional, importação de Decorativos nem Religação ampliada. A pendência de Fechar Projeto da PR #64 permanece separada e não foi corrigida nem aprovada por esta entrega. Não houve push, publicação ou merge.
