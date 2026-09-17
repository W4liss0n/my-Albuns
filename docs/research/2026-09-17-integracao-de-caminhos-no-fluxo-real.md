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

A jornada de interface usa a configuração atual de Exportação. Confere o
cancelamento do seletor nativo de pasta e da configuração, confirma o destino
pelo campo editável e verifica uma única lâmina selecionada, os arquivos exatos
produzidos e o bloqueio de original ausente antes de iniciar o Processador.
O desaparecimento posterior ao congelamento dos bindings continua coberto por
`reopened_project_exports_the_frozen_visible_sheet_through_the_real_processor`
no Host: remove o original, executa o Processador real e exige falha tipada,
orientação para religar e nenhuma saída. O teste de CLI
`processor_identifies_a_missing_original_as_a_source_verification_failure`
confere também a fase e o diagnóstico dessa falha.

## Correção encontrada

Com escrita negada na pasta de destino, a criação da preparação retornava
`ExportStorageUnavailable`, perdendo a causa de permissão. O teste reproduziu
esse resultado antes da correção. Agora `export_io` preserva
`OperationPathAccessDenied`, e a fronteira IPC conserva `pathCode: access_denied`
com orientação para escolher outra pasta ou ajustar a permissão. O código da
fase continua distinto para preparação e publicação; falta de espaço mantém seu
tratamento próprio.

## Travamento encontrado no ensaio nativo

Ao alternar entre dois projetos, a jornada nativa encontrou um bloqueio na
thread principal do Tao 0.35.3. A captura com CDB mostrou `PeekMessageW`
reentrando no callback de teclado enquanto o mutex de `KeyEventBuilder` ainda
estava adquirido. Trazer a janela à frente ou trocar a edição de DPI por eventos
de formulário não eliminou a falha.

A pilha corresponde à causa corrigida no
[PR 1215 do Tao](https://github.com/tauri-apps/tao/pull/1215).
O workspace fixa o commit oficial
`c704261c519c58cfdd0bc2d58ba24e06a0b71c92` por `[patch.crates-io]`, mantendo
a versão compatível 0.35.3 exigida pelo `tauri-runtime-wry` 2.11.4. O teste
continua usando teclado nos campos e confirmações reais. Não foi mantido o
contorno por eventos de formulário.

Em relação à versão publicada 0.35.3, esse commit contém a correção de teclado e
IME no Windows e um ajuste de seis linhas no mapeamento de teclas JIS no Linux.
Não altera APIs ou dependências. O `Cargo.lock` fixa também a origem do macro
associado; as demais resoluções de dependências foram preservadas. Remover esse
override quando a dependência estável do Tauri aceitar uma versão publicada que
contenha a correção. A compilação passa a precisar do repositório Git oficial
na primeira obtenção dessa dependência.

## Abertura enfileirada após cancelamento

O mesmo ensaio encontrou #121: cancelar a decisão sobre uma Cópia externa com
outra ativação na fila podia impedir a próxima abertura. O criador compartilhado
de janelas solicitava a destruição e reutilizava imediatamente o identificador;
o [runtime Wry 2.11.4](https://github.com/tauri-apps/tauri/blob/tauri-runtime-wry-v2.11.4/crates/tauri-runtime-wry/src/lib.rs)
apenas enfileira esse pedido. Agora o criador aguarda assincronamente a retirada
da janela e do WebView dos registros, com limite de cinco segundos, antes de
criar o substituto. Falhas de destruição são propagadas.

A regressão usa o cenário nativo já existente de cancelamento e ativação
enfileirada. Um mock de janela não reproduziria a fila e os registros do Tauri.
O teste não acrescenta atrasos para contornar a disputa.

## Primeiras observações concorrentes

A validação completa revelou #122 no teste público de cargas concorrentes:
uma tentativa recebia conflito de lease e, logo depois, observava o lease
inativo. A sondagem tomava brevemente a trava física sem participar da
arbitragem de publicação. Além disso, a conclusão somente leitura liberava
essa arbitragem antes de descartar a posse ativa criada apenas para concluir
a consulta.

Agora sondagem e aquisição usam o mesmo mutex. A carga somente leitura conserva
o registro durável autorizado e descarta sua posse temporária antes de liberar
a arbitragem, sem publicar uma Sessão. A comparação física e as recusas por
registro corrompido ou indisponível permanecem obrigatórias. Isso cumpre a
serialização de primeiras observações exigida pelo design 0015.

Um teste controla a publicação pendente e verifica que a sondagem espera sua
conclusão: falhou antes da correção e passou depois. Os 61 testes públicos de
persistência passaram, seguidos de 500 execuções do cenário concorrente. A
carga somente leitura também é verificada com armazenamento real de identidade
e abertura editável posterior, sem posse residual.

## Coleta da evidência nativa

O coletor acompanha o schema 12 e os diálogos atuais de Importação e Exportação.
Dois problemas do próprio ensaio foram corrigidos sem alterar o produto: o
Windows PowerShell 5.1 interpretava o caminho acentuado do JPEG como ANSI, e a
comparação visual usava coordenadas anteriores ao layout atual. O caminho agora
constrói os caracteres acentuados explicitamente. A medição usa o centro do
Frame persistido e a posição da lâmina renderizada, considerando escala,
navegação e sangria.

A comparação exige transformação neutra e ponto visível no Canvas; continua
usando tolerância de 32 níveis por canal. A execução final apresentou diferença
máxima de 1 entre Canvas e JPEG, e 8 entre Original e JPEG. O Background também
ficou a 1 nível do valor persistido. As imagens retidas foram inspecionadas.

## Resultado final em 17 de setembro de 2026

| Validação | Revisão executada | Resultado | Evidência local |
| --- | --- | --- | --- |
| `npm run validate` | `d6afdfe4` | Sete etapas aprovadas; 1.350 testes de frontend, 120 de automação e 970 resultados de testes Rust aprovados; três testes opcionais de automação ignorados | `.tools/windows-path-integration/validation-final/report.json` e logs associados |
| `npm run test:windows-path -- -AllowVisibleWindows` | `d6afdfe4` | 12 verificações aprovadas; origem limpa | `.tools/windows-path-integration/windows-path-gate-final.json` |
| `npm run test:productive-journey -- -AllowVisibleWindows` | `468cac71` | 33 verificações aprovadas, das quais 26 nativas, uma pela API pública Rust e seis de componentes; origem limpa | `.tools/windows-path-integration/native-journey.json` |

O relatório completo conserva `sourceInputsDirty: true`: na partida, o índice
ainda indicava dois arquivos após a remoção de sondas temporárias, embora seu
diff normalizado estivesse vazio. Atualizar essas entradas no índice não
produziu alteração para commit; o HEAD permaneceu `d6afdfe4`, sem edição de
produção durante a execução. Essa ocorrência está descrita no `source-note.md`
junto ao relatório e não é apresentada como uma execução de origem limpa.
As duas validações seguintes registraram `sourceInputsDirty: false`.

A jornada final exportou apenas a lâmina 2 em JPEG de 1.440 × 360 pixels,
preservou o projeto e o Original, verificou ausência de Cache antes e depois da
exportação e encerrou os processos de teste. Os registros correlacionam cinco
aberturas e uma execução do Processador. Seus 44 artefatos estão em
`.scratch/productive-journey-evidence/windows-path-integration-final-r6`, com
manifesto e hashes. Os 25 testes do coletor também passaram após seu último
ajuste. As revisões de padrões e aderência à especificação não deixaram achados
pendentes.

## Limites

O SMB do ensaio é local. A indisponibilidade é provocada pelo desaparecimento do
binding operacional e pela remoção do mapeamento; não representa todos os modos
de falha de servidores, VPNs ou redes físicas. O teste de interface complementar
é `npm run test:productive-journey -- -AllowVisibleWindows`, com diálogos nativos
e arquivos isolados. Os relatórios distinguem origem limpa de execução sobre
arquivos ainda em edição.

Uma execução anterior registrou `Project(Path(IoFailure))` ao salvar um projeto
independente, sem conservar a causa bruta do sistema operacional. A ocorrência
não reapareceu nas jornadas posteriores nem em 100 repetições do teste público
de salvamento. Permanece uma hipótese sem reprodução determinística; não é
atribuída a #122 nem declarada corrigida. O registro original foi preservado em
`.scratch/productive-journey-evidence/windows-path-integration-final-r3`.

## Documentação técnica consultada

- [GetFinalPathNameByHandleW — Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew): caminho físico pelo handle e limitações de normalização em SMB.
- [Set-Acl — Windows PowerShell 5.1](https://github.com/MicrosoftDocs/PowerShell-Docs/blob/main/reference/5.1/Microsoft.PowerShell.Security/Set-Acl.md): alteração do descritor de segurança de uma fixture por caminho literal.
- [Cargo — substituição de dependências](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html): correção transitiva fixada no manifesto do workspace.
- [LockFileEx — Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex): a trava exclusiva também impede outra aquisição por um handle do mesmo processo.
- [Codificação de caracteres — PowerShell](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_character_encoding?view=powershell-7.6): Windows PowerShell interpreta scripts sem BOM segundo a página de códigos ANSI.
