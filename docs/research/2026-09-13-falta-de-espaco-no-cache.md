---
status: current
document: research
date: 2026-09-13
ticket: 96
---

# Falta de espaço na preparação do Cache

## Falha reproduzida

A publicação do índice removia o arquivo anterior antes de renomear o temporário.
Ao simular falta de espaço nessa renomeação, o teste de CacheEngine encontrou o
índice anterior ausente (`NotFound`). O Projeto e a foto original mantiveram os
mesmos bytes, mas o Cache perdia seu índice válido.

O teste usa um Projeto real criado pelo Core, arquivos locais temporários e o
fluxo de preparação/publicação do CacheEngine. A falha é aplicada somente à
operação de sistema de arquivos de um destino exato, sem preencher um disco e
sem alterar o Cache do usuário. O transporte do Processador é controlado nesse
teste; a codificação real de imagens é exercitada separadamente no Processador.

## Correção

A substituição acontece na renomeação protegida pelo diretório já autorizado.
O destino anterior é validado e permanece no lugar até a operação ter sucesso.
Falhas na gravação ou renomeação descartam somente o temporário e a geração
candidata. O índice anterior e a geração nele referenciada permanecem intactos.

No Windows, `FILE_RENAME_INFORMATION.ReplaceIfExists` permite que a própria
renomeação substitua o destino. A operação continua relativa ao handle do
diretório autorizado. As operações que exigem um destino novo continuam sem
autorizar substituição. [Contrato da Microsoft](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information).

A classificação de falta de espaço considera `StorageFull`, esgotamento de cota
e os códigos nativos correspondentes. Ela não depende do idioma da mensagem
do sistema. [Erros de I/O do Rust](https://doc.rust-lang.org/std/io/enum.ErrorKind.html),
[códigos Win32](https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--0-499-)
e [NTSTATUS](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-erref/596a1078-e883-4972-9bbc-49e60bebca55).

O código instalado de `image 0.25.10` e `png 0.18.1`, nas versões fixadas pelo
`Cargo.lock`, mostra que a conversão do erro do encoder PNG para `io::Error`
preserva apenas seu texto. Por isso, a preparação acompanha o erro no escritor
real antes dessa conversão. O teste de PNG reproduziu e validou esse caso.

## Comportamento observável

- JPG e PNG continuam fornecendo dimensões e identificação do original quando
  somente a criação do Cache falha; outras imagens do lote continuam.
- A falha de disco cheio do comando de Cache usa a saída determinística `30`,
  sem reiniciar nem suspender o Processador.
- O erro orienta: “Não há espaço para preparar as imagens. Libere espaço no disco
  e abra o álbum novamente.”
- Avisos disparados pela demanda de miniaturas não reaparecem a cada imagem no
  mesmo Projeto aberto, depois de dispensados.
- Após remover a falha simulada, uma nova geração é publicada normalmente.

## Fronteiras verificadas

- CacheEngine: gravação parcial do índice e falha na renomeação final; comparação
  dos bytes do Projeto, original, índice e miniatura anterior; descarte dos
  candidatos e sucesso da tentativa seguinte.
- Preparação de importação: codificação real de JPG e PNG, progresso até o final
  do lote, preservação dos originais e nova preparação após liberar espaço.
- Comando de Cache do Processador: falha determinística tipada na gravação real.
- Aplicação: apresentação do aviso pelo diálogo existente, descarte de avisos
  repetidos e manutenção da edição/Exportação após dispensá-lo.

Os pontos de falha ficam atrás de `test-support`, habilitado somente pelas
dependências de testes; não existe variável de ambiente que os ative no programa.

Os testes locais passaram: 436 casos do Host, 152 casos de Processador,
protocolo e caminhos, e 84 casos da aplicação e adaptação IPC. Os testes auxiliares
marcados como ignorados pelos próprios pacotes não foram contabilizados como
aprovações. O contrato IPC foi regenerado a partir do Rust.
TypeScript, build da interface, formatação Rust e Clippy também passaram.

Esta verificação cobre falhas de gravação e publicação. Não equivale a um teste
manual com um volume físico cheio. Depois dessa verificação, em 13 de setembro
de 2026, o usuário dispensou o aviso preventivo: somente falhas reais de falta de
espaço devem gerar aviso, conforme o design de armazenamento atualizado.
A migração final de namespace continua em #16/#40. A decisão de manter a limpeza
em segundo plano permanece inalterada.
