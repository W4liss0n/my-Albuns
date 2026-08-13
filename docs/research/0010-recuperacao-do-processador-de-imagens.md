---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-29
updated: 2026-08-12
---

# Recuperação do Processador de Imagens

## Resumo

Este gate demonstra a recuperação segura do Processador de Imagens durante a
geração de Cache e a falha controlada durante a Exportação. A rodada canônica
atual também atravessa cancelamento obsoleto, pausa causal e o Canvas real do
aplicativo Tauri/WebView2. Os cenários encerram processos reais depois que uma
preparação parcial aparece no disco e derivam a evidência de PIDs, arquivos e
hashes efetivamente observados.

Durante Cache, somente o temporário pertencente ao PID encerrado é descartado e
o pedido ainda relevante é repetido uma vez em outro processo. Durante
Exportação, não há repetição automática: o arquivo publicado anterior permanece
intacto, a resposta incompleta não é aceita como sucesso e uma nova tentativa
explícita usa outra preparação e outro processo.

Os dados brutos da rodada mais recente estão em
[0004-imaging-recovery.json](artifacts/0004-imaging-recovery.json). O JSON é a
fonte canônica do commit, dos PIDs, dos hashes e dos tempos daquela execução. A
primeira versão desta pesquisa descrevia uma prova de quatro grupos; essa prova
foi substituída pelo gate posterior de 9 verificações e não representa a mesma
execução do artefato atual.

O campo `sourceInputsDirty` considera todo arquivo rastreado e também arquivos
novos não rastreados que não estejam ignorados pelo Git, inclusive
configurações e entradas na raiz. Somente o próprio arquivo de evidência é
excluído; outputs de build declarados no `.gitignore` ficam fora da árvore de
fontes. Assim, `sourceInputsDirty: false` atribui a execução ao `gitCommit`
registrado sem esconder alterações locais de Vite, TypeScript, Tauri ou
manifests. O runner captura commit e estado antes do primeiro check, repete a
captura depois do último e só publica `false` quando o HEAD permaneceu o mesmo e
as duas observações estavam limpas.

## Fronteiras implementadas

A recuperação mantém responsabilidades separadas:

- `imaging_processor` define a fronteira tipada comum à aplicação e à prova
  integrada. O adaptador Tauri usado pela aplicação e o adaptador
  `std::process` exclusivo do teste compartilham codec, correlação,
  cancelamento, progresso e classificação da terminação;
- no protocolo v17, o host conserva o handle da tentativa até confirmar o
  término após cancelamento ou atingir um limite explícito. Uma terminação não
  confirmada recebe classificação própria e coloca o Processador em
  quarentena;
- `CacheEngine` possui demanda, prioridades, single-flight, gerações,
  publicação, invalidação e recuperação. Em uma queda inesperada, remove
  somente nomes terminados pelo PID encerrado e reinicia uma vez apenas quando
  a demanda continua atual;
- `CacheActivityGate` e `OperationLease` tornam a pausa causal: fecham a entrada
  de novos trabalhos, cancelam o Cache ativo, aguardam seu endpoint seguro,
  reservam o Processador para a Exportação e retomam depois de todo terminal;
- `ExportPipeline` possui planejamento, preparação, validação, publicação e
  descarte. Cada tentativa invoca o transporte uma vez e não oferece repetição
  automática;
- `AppPaths` mantém as cadeias de diretórios validadas e restringe descarte e
  publicação à tentativa;
- `MyAlbuns.Imaging.exe` é o único participante que abre Originais e escreve
  representações ou saídas derivadas. Ele recebe o comando tipado e o
  `RootBindingPlan` imutável, sem conhecer o caminho do arquivo de Projeto;
- o host confere tamanho, SHA-256 e contrato da preparação antes de publicar.
  Nenhuma verificação falível ocorre depois da Publicação;
- o protocolo opaco do WebView serve somente bytes derivados publicados. O
  pathname e os bytes do Original não atravessam para o Canvas.

A sessão usada no ensaio é serializada antes e depois da queda; os dois
SHA-256 são idênticos. A Exportação final continua usando `RenderSnapshot`,
`CompositionPlan` e Originais exatos, nunca o Cache.

## Método

O comando `npm run test:imaging-recovery` executa 9 verificações:

1. contrato serializado e códigos de falha do protocolo;
2. descarte seletivo de temporários de Cache;
3. build isolado do executável real do Processador;
4. recuperação produtiva com queda durante Cache e Exportação;
5. jornada Cache–Canvas–Exportação com Background e Overlay;
6. cancelamento de uma demanda de Cache obsoleta;
7. pausa causal do Cache durante uma Exportação;
8. build real da aplicação Tauri para WebView2;
9. direção do `AlbumCanvas`/Pixi produtivo por `tauri-driver`.

O build do Processador usa um target isolado por execução. Isso impede que um
binário incremental obsoleto do Cargo seja confundido com o executável recém-
compilado. O diretório é descartado no `finally` do runner.

O teste integrado espera cada arquivo parcial aparecer enquanto o processo
ainda está vivo, encerra exatamente esse processo e aguarda sua coleta. O gate
falha se os PIDs forem iguais, se o temporário não for observado ou removido,
se um temporário alheio for removido, se o índice aparecer antes da conclusão,
se houver sucesso na tentativa incompleta ou se os hashes protegidos mudarem.

## Resultado

| Evidência estável | Resultado observado na rodada canônica |
|---|---|
| Processo de Cache após a queda | reiniciado com outro PID |
| Temporário do PID encerrado | observado e removido |
| Temporário de outro PID | preservado |
| `metadata.json` | ausente após a queda e presente após o reinício |
| Exportação antes da nova tentativa | sem resposta de sucesso |
| Saída publicada anterior | SHA-256 preservado |
| Revisão do Projeto | SHA-256 preservado |
| Nova tentativa explícita | publicada por outro processo |
| Demanda obsoleta | processo cancelado e coletado, sem índice |
| Pausa causal | Cache bloqueado e Processador exclusivo |
| Retomada após Exportação | nova geração publicada |
| WebView | Tauri/WebView2, `AlbumCanvas` e Pixi reais |
| Recursos do Canvas | dois tokens opacos, sem Original exposto |
| Fidelidade Canvas/Exportação | delta máximo dentro do limiar do gate |

Os valores voláteis exatos permanecem apenas no JSON canônico para não
apresentar PIDs, hashes ou tempos de rodadas distintas como uma única execução.

## Conclusão do gate

Fica demonstrado o reinício seguro do Processador durante Cache e a falha
controlada durante Exportação, sem corromper o Projeto nem anunciar uma
Publicação incompleta. A expansão da rodada também cobre os critérios do
Programa 03A para cancelamento, pausa causal, recuperação de demanda relevante
e fidelidade do Canvas real.

## Limites da conclusão

- o gate não cobre queda completa do processo principal nem Recuperação de
  sessão;
- ele não implementa promoção de Identidade, Cópia externa, Movimentação,
  `Salvar como` ou limpeza global do Cache;
- o corpus e as políticas de decode pertencem ao spike específico de
  representação reduzida e devem ser repetidos quando crates ou codecs mudarem;
- versões futuras de WebView2, Tauri ou do protocolo exigem nova rodada.

## Repetição

```powershell
npm run test:imaging-recovery
```

O comando reexecuta processos e WebView2 reais, valida as relações observadas e
substitui o artefato JSON somente depois que todos os checks passam.
