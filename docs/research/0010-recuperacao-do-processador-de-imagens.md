---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-29
updated: 2026-07-29
---

# Recuperação do Processador de Imagens

## Resumo

Este gate demonstra reinício seguro do Processador de Imagens durante Cache e
falha controlada durante Exportação. As duas provas encerram processos reais
depois que uma preparação parcial aparece no disco e derivam a evidência de
PIDs, arquivos e hashes efetivamente observados.

Durante Cache, somente o temporário pertencente ao PID encerrado é descartado e
o pedido relevante é repetido uma vez em outro processo. Durante Exportação,
não há repetição automática: o arquivo publicado anterior permanece intacto, a
resposta incompleta não é aceita como sucesso e uma nova tentativa explícita
usa outra preparação e outro processo.

Os dados brutos estão em
[0004-imaging-recovery.json](artifacts/0004-imaging-recovery.json). O artefato
identifica o commit exato dos insumos e confirma `sourceInputsDirty: false`.

## Fronteiras implementadas

A recuperação mantém responsabilidades separadas:

- `imaging_processor` transporta uma única mensagem, registra tentativa e PID e
  devolve falhas tipadas; ele não decide repetição nem ciclo de arquivos;
- `CacheEngine` possui a política de Cache: em uma queda inesperada, remove
  somente nomes terminados pelo PID encerrado e reinicia uma vez; uma falha
  determinística não é repetida e uma segunda queda termina a operação depois
  de limpar seu próprio temporário;
- `ExportPipeline` possui preparação, validação, publicação e descarte; seu
  invocador é de uso único e, portanto, não oferece repetição automática;
- `AppPaths` deriva uma pasta única
  `.myalbuns-export-{operation-id}.tmp` dentro do Destino, mantém a cadeia de
  diretórios validada e restringe descarte e publicação à tentativa;
- `MyAlbuns.Imaging.exe` recebe no protocolo v3 somente o caminho do arquivo
  preparado. Ele grava, sincroniza, calcula tamanho e SHA-256 e responde sem
  conhecer ou promover o nome final;
- o host confere novamente tamanho e SHA-256 da preparação antes de pedir a
  promoção. Nenhuma verificação falível ocorre depois da Publicação.

O protocolo continua sem caminho de arquivo de Projeto e leva somente valores
imutáveis. A sessão usada no ensaio foi serializada antes e depois da queda; os
dois SHA-256 são idênticos.

## Método

O comando `npm run spike:imaging-recovery` executou cinco grupos de
verificação:

1. contrato serializado e códigos de falha do protocolo;
2. descarte seletivo de temporários de Cache, preservando um temporário
   pertencente a outro PID;
3. políticas reais de `CacheEngine` e `ExportPipeline`, incluindo falha de
   validação antes da Publicação;
4. queda do processo durante a geração de uma representação JPEG a partir de
   uma Foto real de 4096 × 3072 px;
5. queda do processo durante uma Exportação a 300 DPI com original JPEG
   vinculado, seguida somente depois por nova tentativa explícita.

O teste espera o arquivo parcial aparecer enquanto o processo ainda está vivo,
encerra exatamente esse processo e aguarda sua coleta. O relatório falha se os
PIDs forem iguais, se o temporário não for observado ou removido, se o índice
de Cache aparecer antes da conclusão, se houver resposta de sucesso na
tentativa incompleta ou se os hashes protegidos mudarem.

## Resultado

| Evidência | Resultado observado |
|---|---|
| Processo de Cache encerrado | PID `25652` |
| Processo reiniciado para Cache | PID `4756` |
| Temporários do PID encerrado removidos | `1` |
| `metadata.json` depois da queda | ausente |
| `metadata.json` depois do reinício | presente |
| Processo de Exportação encerrado | PID `3204` |
| Resposta de sucesso antes da nova tentativa | não |
| Política de fonte | `linkedOriginals` |
| Processo da tentativa explícita | PID `9820` |

O SHA-256 da Exportação publicada anterior permaneceu
`30ea0007992ce2ad6109353b8683631daa9946cffbde801d0979f327a98b9c79`
antes e depois da queda. O SHA-256 da revisão do Projeto permaneceu
`1fa39a7104b6080a3397beb98dd073295e9bbaf07886f5593b5baa4a955d6c70`.
A tentativa explícita publicou outro PNG, com SHA-256
`bf96043e2672584027dd540bb7cccf84169c82d330a3b82592a9a4825ca9d945`.

## Conclusão do gate

Fica atendido o critério do ticket 01 que exige reinício seguro do Processador
durante Cache e falha controlada durante Exportação, sem corromper o Projeto
nem anunciar uma Publicação incompleta como sucesso.

O comportamento pertence aos mesmos módulos nas duas topologias candidatas e
não depende de uma segunda cópia mutável do Projeto. Isso não encerra o gate
mais amplo de queda do processo principal e dos hosts, nem escolhe a topologia.
Também permanecem abertos `OperationGate`, `OperationLease`, cancelamento,
progresso, recuperação de órfãos após queda do próprio host, bindings de raiz e
Exportação para UNC.

## Repetição

```powershell
npm run spike:imaging-recovery
```

O comando reexecuta as quedas reais, valida as relações observadas e substitui
o artefato JSON somente depois que todos os checks passam.
