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
falha controlada durante Exportação. Os dois cenários encerram processos reais
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

- `imaging_processor` define a fronteira tipada comum à aplicação e à prova
  integrada. O adaptador Tauri usado pela aplicação e o adaptador
  `std::process` exclusivo do teste compartilham o codec, a correlação e a
  classificação da terminação. No protocolo v6, o adaptador Tauri também
  encaminha progresso tipado conforme o fluxo é produzido e conserva o handle
  da tentativa até observar o término após um cancelamento ou atingir um limite
  explícito. Uma terminação não confirmada recebe classificação própria;
  somente a inicialização e a coleta de eventos, dependentes de cada runtime,
  são distintas. A fronteira não decide repetição nem ciclo de arquivos;
- `CacheEngine` possui a política de Cache: em uma queda inesperada, remove
  somente nomes terminados pelo PID encerrado e reinicia uma vez; uma falha
  determinística não é repetida e uma segunda queda termina a operação depois
  de limpar seu próprio temporário. Ele também deriva gerações, valida
  artefatos e publica `metadata.json`;
- `ExportPipeline` possui planejamento, preparação, validação, publicação e
  descarte; cada execução invoca o transporte uma vez e, portanto, não oferece
  repetição automática. Se o encerramento do Processador não puder ser
  confirmado, conserva a preparação possivelmente ainda em uso em vez de
  tentar removê-la;
- `AppPaths` deriva uma pasta única
  `.myalbuns-export-{operation-id}.tmp` dentro do Destino, mantém a cadeia de
  diretórios validada e restringe descarte e publicação à tentativa;
- `MyAlbuns.Imaging.exe` recebe no protocolo v6 somente o comando tipado, o
  `RootBindingPlan` imutável da tentativa e os
  caminhos preparados. Ele grava, sincroniza, calcula tamanho e SHA-256 e
  responde sem
  conhecer ou promover o nome final;
- o host confere novamente tamanho e SHA-256 da preparação antes de pedir a
  promoção. Nenhuma verificação falível ocorre depois da Publicação.

O protocolo continua sem caminho de arquivo de Projeto e leva somente valores
imutáveis. A sessão usada no ensaio foi serializada antes e depois da queda; os
dois SHA-256 são idênticos.

## Método

O comando `npm run spike:imaging-recovery` executou quatro grupos de
verificação:

1. contrato serializado e códigos de falha do protocolo;
2. descarte seletivo de temporários de Cache, preservando um temporário
   pertencente a outro PID;
3. compilação do executável real do Processador de Imagens;
4. uma prova integrada que atravessa a mesma fronteira `ImagingTransport`, o
   `CacheEngine` e o `ExportPipeline` usados em produção: ela encerra o
   processo durante a geração de uma representação JPEG a partir de uma Foto
   real e durante uma
   Exportação a 300 DPI, seguida somente depois por nova tentativa explícita.

O mesmo teste integrado espera cada arquivo parcial aparecer enquanto o
processo ainda está vivo, encerra exatamente esse processo e aguarda sua
coleta. O relatório falha se os PIDs forem iguais, se o temporário não for
observado ou removido, se um temporário de outro PID for removido, se o índice
de Cache aparecer antes da conclusão, se houver resposta de sucesso na
tentativa incompleta ou se os hashes protegidos mudarem.

## Resultado

| Evidência | Resultado observado |
|---|---|
| Processo de Cache encerrado | PID `12088` |
| Processo reiniciado para Cache | PID `26140` |
| Temporários do PID encerrado removidos | `1` |
| Temporário de outro PID preservado | sim |
| `metadata.json` depois da queda | ausente |
| `metadata.json` depois do reinício | presente |
| Processo de Exportação encerrado | PID `10276` |
| Resposta de sucesso antes da nova tentativa | não |
| Política de fonte | `linkedOriginals` |
| Processo da tentativa explícita | PID `29764` |

O SHA-256 da Exportação publicada anterior permaneceu
`30ea0007992ce2ad6109353b8683631daa9946cffbde801d0979f327a98b9c79`
antes e depois da queda. O SHA-256 da revisão do Projeto permaneceu
`1fa39a7104b6080a3397beb98dd073295e9bbaf07886f5593b5baa4a955d6c70`.
A tentativa explícita publicou outro PNG, com SHA-256
`845bb7b85113c9b1d60b96a51832dea148422bbbbfdedf617d5d0fd361addabb`.

## Conclusão do gate

Fica atendido o critério do ticket 01 que exige reinício seguro do Processador
durante Cache e falha controlada durante Exportação, sem corromper o Projeto
nem anunciar uma Publicação incompleta como sucesso.

O comportamento pertence aos mesmos módulos nas duas topologias candidatas e
não depende de uma segunda cópia mutável do Projeto. Isso não encerra o gate
mais amplo de queda do processo principal e dos hosts, nem escolhe a topologia.
Também permanecem abertos `OperationGate`, `OperationLease`, a conexão do
cancelamento e do progresso da tentativa com a janela do produto, recuperação
de órfãos após queda do próprio host, bindings reais de raiz e Exportação para
UNC.

## Repetição

```powershell
npm run spike:imaging-recovery
```

O comando reexecuta as quedas reais, valida as relações observadas e substitui
o artefato JSON somente depois que todos os checks passam.
