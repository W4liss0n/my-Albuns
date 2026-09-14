---
status: current
document: research
date: 2026-09-13
ticket: 38
---

# Verificação da limpeza de saídas órfãs

## Alcance

A limpeza já integra a Exportação normal. Esta entrega verifica a pendência #38,
acrescenta cobertura de regressão e concilia a especificação com as decisões de
interface aceitas pelo usuário. Não altera o código de produção nem inicia a
Exportação em lote (#39).

A confirmação continua genérica, com `Ignorar`, `Substituir` e `Cancelar`, sem
listar arquivos. A seleção parcial não mostra um aviso adicional sobre arquivos
fora do intervalo ou sobre o modo da exportação anterior. Esses pedidos posteriores
substituem os antigos critérios de interface da #38; a preservação dos arquivos e
a autorização para limpeza permanecem regidas pelos ADRs 0003 e 0006.

## Caminho de produção

`ExportPreviewControl` recebe os conflitos e encaminha a decisão ao comando de
Exportação normal. Em `src-tauri/src/export_commands/normal.rs`, o planejamento
ocorre antes da execução. `AlbumExportPlan::conflicts` inclui tanto as saídas
atuais existentes quanto os candidatos órfãos; portanto, um destino contendo
somente órfãos também exige a decisão do usuário.

Em `src-tauri/src/export_pipeline/album.rs`, a identificação percorre apenas a
pasta de destino e exige Nome, extensão e índice positivo canônico. `Substituir`
autoriza a limpeza somente em uma Exportação integral JPEG/PNG. `Ignorar` limpa
essa autorização e preserva os arquivos existentes. PDF e intervalos não removem
saídas excedentes.

O pipeline prepara e verifica todas as saídas antes de publicar. Somente depois da
publicação de todo o conjunto ele chama a remoção dos candidatos em
`crates/myalbuns-paths/src/export.rs`. Os caminhos são limitados ao destino e
comparados com os Originais protegidos. Uma falha de preparação ou publicação
preserva os órfãos; uma publicação parcial informa a quantidade já confirmada.
Não há manifesto ou rollback do conjunto.

## Evidência

O novo teste
`complete_image_exports_clean_only_their_namespace_after_reduction_and_mode_changes`
executa a sequência completa em JPEG e PNG:

| Tentativa | Lâminas | Modo | Saídas ao concluir |
| --- | ---: | --- | ---: |
| Primeira exportação | 36 | Por lâmina | 36 |
| Redução do álbum | 34 | Por lâmina | 34 |
| Mudança de modo | 34 | Por página | 68 |
| Retorno ao modo anterior | 34 | Por lâmina | 34 |

Em cada substituição, um arquivo manual `Album_1000` com a extensão selecionada
é indistinguível de uma saída antiga: permanece durante toda a publicação e é
removido ao concluir. Outros nomes, outra extensão, índices fora da convenção,
arquivos em subpastas e o Original mantêm seus bytes. A contagem final verifica
que não restam saídas excedentes, preparação ou manifesto.

O teste existente de preparação também verifica o conflito quando o destino
contém somente um órfão. Os demais testes do pipeline cobrem falhas na preparação
e na publicação, falta real de espaço, nova tentativa, intervalo parcial,
`Ignorar`, PDF, conflito com diretório e proteção de Originais fora da seleção.
Os testes de interface existentes verificam a confirmação genérica e o
encaminhamento das três decisões, preservando as opções da tentativa.

Os casos do Host usam diretórios e publicação reais com transporte controlado do
Processador. Eles não demonstram a codificação final das imagens nem a aparência
de janelas nativas. Não houve alteração visual nesta entrega.

## Validação

Os resultados finais dos testes, revisão e integração são registrados na PR
vinculada à #38. Os testes direcionados executam o módulo
`export_pipeline::tests`; a validação completa da PR inclui também os fluxos com
o Processador real e as verificações de qualidade do repositório.
