---
status: current
document: research
date: 2026-09-17
ticket: 13
platform: windows
---

# Integração de caminhos na criação, abertura e exportação

A integração usa o contrato de caminhos existente no bootstrap, no
`ProjectStore`, no Host e no Processador. O namespace temporário continua
`MyAlbuns2`; sua migração pertence ao ticket #40.

## Cobertura executável

O comando `npm run test:windows-path -- -AllowVisibleWindows` reúne as provas
abaixo. Requer Windows e um compartilhamento SMB acessível. Por padrão, usa o
compartilhamento administrativo da própria máquina e uma unidade livre.

| Critério de #13 | Fronteira verificada |
| --- | --- |
| Pai aberto, filho validado e contenção física | `path_resolution`: preparação de destino e conferência do filho pelo handle; `project_document_v1`: criação e persistência pelo Core |
| Formas proibidas, tipo incorreto e escape físico | `path_resolution` e `app_paths`: sintaxe, namespaces, reparse points, arquivo versus diretório |
| Resultados tipados e trabalho fora da interface | testes do bootstrap, `path_io` e conversão de falhas da exportação para IPC |
| Criação, salvamento, reabertura e exportação reais | `project_bootstrap::host::tests::windows_paths`: sete variantes de caminho, documento persistido e JPEG produzido pelo Processador real |
| UNC, unidade mapeada, aliases, remapeamento e retry | `windows_path_gate` e `real_processor_consumes_the_frozen_unc_plan_after_the_drive_is_unmapped` |
| Não ASCII, verbatim e mais de 260 caracteres | matriz do Host: local, UNC, unidade mapeada, VerbatimDisk, VerbatimUNC, local longo e UNC longo |
| Permissão real e nova tentativa | ACL temporária impede criação, leitura do projeto ou preparação da exportação; a mesma operação funciona após remover a restrição |
| Cache local, preparação no destino e plano compartilhado | evidências do gate de caminhos e do Processador, incluindo digest do plano |

A matriz nova passa pela mesma exportação de álbum usada pelo programa,
selecionando uma lâmina. Confere dimensões do JPEG, preservação do projeto e
original e remoção da preparação. Não acrescenta comandos de diagnóstico ao
produto. Os testes de ACL só alteram arquivos e pastas criados pelo próprio
ensaio, restaurando a regra mesmo em falhas de asserção.

## Correção encontrada

Com escrita negada na pasta de destino, a criação da preparação retornava
`ExportStorageUnavailable`, perdendo a causa de permissão. O teste reproduziu
esse resultado antes da correção. Agora `export_io` preserva
`OperationPathAccessDenied`, e a fronteira IPC conserva `pathCode: access_denied`
com orientação para escolher outra pasta ou ajustar a permissão. O código da
fase continua distinto para preparação e publicação; falta de espaço mantém seu
tratamento próprio.

## Limites

O SMB do ensaio é local. A indisponibilidade é provocada pelo desaparecimento do
binding operacional e pela remoção do mapeamento; não representa todos os modos
de falha de servidores, VPNs ou redes físicas. O teste de interface complementar
é `npm run test:productive-journey -- -AllowVisibleWindows`, com diálogos nativos
e arquivos isolados. Os relatórios distinguem origem limpa de execução sobre
arquivos ainda em edição.

## Documentação técnica consultada

- [GetFinalPathNameByHandleW — Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew): caminho físico pelo handle e limitações de normalização em SMB.
- [Set-Acl — Windows PowerShell 5.1](https://github.com/MicrosoftDocs/PowerShell-Docs/blob/main/reference/5.1/Microsoft.PowerShell.Security/Set-Acl.md): alteração do descritor de segurança de uma fixture por caminho literal.
