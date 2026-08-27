---
status: current
document: technical-research
ticket: 17-programa-09-primeira-composicao-com-foto
date: 2026-08-20
updated: 2026-08-21
platform: windows-11-x64
---

# Primeira composição com Foto

## Pergunta

Como acrescentar a primeira Foto editável sem duplicar autoridade entre
Canvas, Host, Cache e Processador, e como provar que o JPEG final continua
dependendo do Original?

## APIs e versões verificadas

A implementação foi decidida contra as versões resolvidas no repositório:
Tauri 2.11.5, `tauri-plugin-dialog` 2.7.2, PixiJS 8.19, React 19.1 e `image`
0.25.10. A documentação atual confirmou três pontos usados pelo corte:

- o diálogo do Tauri pode ser configurado no Host com janela proprietária,
  título, filtro e função de retorno, mantendo o caminho fora da WebView;
- no PixiJS 8, `eventMode = "static"` e `hitArea` retangular fornecem a
  superfície explícita de interação, enquanto a escolha autoritativa do alvo
  continua no domínio;
- a biblioteca `image` identifica o formato pelo fluxo de bytes e expõe dimensões e
  orientação; o Host troca os eixos observados quando necessário, enquanto o
  Processador continua proprietário da orientação efetiva dos pixels.

Nenhuma dependência nova foi necessária. O codec JPEG, o diálogo, o Canvas e o
protocolo do Processador já pertenciam à árvore aceita.

## Corte adotado

A menor fronteira persistente é `Frame + PhotoTransform`. Metadados do arquivo
são reidratados pelo Monitor depois de `host_ready` e da inscrição da WebView
em `myalbuns://linked-media-changed`. A confirmação `project_ui_ready` fecha
essa barreira causal e concede uma única inicialização do Monitor. A inspeção
ocorre no executor de trabalho bloqueante, fora do fluxo de execução da inicialização, e
`baseFillZoom` é calculado pelo `CompositionEngine`.
Isso evita gravar propriedades derivadas e permite que uma mudança estável do
Original atualize a composição sem revisão ou Histórico. A execução indexa cada
observação por `mediaId + path`; assim, Religação, Undo e Redo não reutilizam as
dimensões observadas para outro vínculo lógico.

Resolver o alvo é uma consulta pura do `ProjectCore`: valida a superfície e
varre Frames da frente para trás. A soltura envia coordenadas, não uma decisão
da WebView. A mesma função é usada para destaque e mutação; a WebView só aceita
uma soltura quando mídia, ponto e geração coincidem com o destaque visível. Respostas
de projeção do Monitor também têm geração monotônica, inclusive quando a revisão
criativa não muda.

O modo normal reutiliza um único primeiro Layout determinístico, enquanto o
Modo de edição cria apenas o retângulo proporcional pedido. Não foi criada uma
infraestrutura de Layouts, um repositório de seleção ou uma segunda pilha de Histórico.

Selecionar novamente o mesmo JPEG é tratado na fronteira pública de importação: o
Core encontra a ocorrência pelo par `kind + path`, atualiza somente os
metadados observados e devolve o `mediaId` existente. A WebView seleciona esse
cartão sem revisão criativa ou entrada adicional de Undo/Redo.

Na reabertura, o namespace de Cache reservado valida índice e artefatos reais
antes de expor qualquer contexto. Somente uma geração cujo caminho, tamanho,
codec, dimensões, perfil sRGB e decodificação foram confirmados pode reidratar as
dimensões já orientadas da Foto antes da primeira Projeção. Isso evita o
estado geométrico de contingência 1×1 quando o Original já está ausente, sem transformar o
Cache em autoridade para Exportação; índice ou geração corrompidos são
descartados como estado derivado.

## Protocolo de fidelidade e ausência

`scripts/Test-ProductiveJourney.ps1` usa uma Foto JPEG externa real, seleciona
o arquivo em diálogo nativo, seleciona novamente o mesmo arquivo e aciona o
duplo clique pela WebView2. A rodada confirma que a segunda seleção reutiliza o
cartão sem revisão, esquema v3 e vínculo externo único sem metadados derivados,
materializa a textura opaca no Canvas, salva, reabre em outro Host e exporta
pelo Processador.

A amostra central da Foto é comparada entre captura do Canvas e JPEG final com
tolerância máxima explícita de 32 níveis por canal, cobrindo a recompressão
JPEG. Uma amostra fora do Frame mantém tolerância 8 para o Background. O executor
também verifica o hash e os bytes do Original antes e depois das operações.

A jornada confirma primeiro uma representação JPEG real no namespace de Cache
isolado do processo. Enquanto o diálogo nativo da primeira Exportação ainda está
aberto, remove somente esse conteúdo derivado por uma travessia que falha fechada e
mede zero entradas e zero bytes. O aplicativo real exporta pelo Original; o
namespace continua com zero entradas e zero bytes depois do Processador. Antes
de remover qualquer entrada, o executor verifica por caminho real e atributos que
a raiz e todos os seus ancestrais não são junções nem pontos de nova análise. Dois testes
Windows criam junções reais na raiz e em um ancestral e provam que uma
sentinela externa permanece intacta.

Em seguida, a textura já residente continua visível no Canvas, mas o executor
remove o Original antes da segunda Exportação e exige mensagem com
`Religar`/`Religue`, terminal `export_failed` em `source_verification` e nenhum
arquivo publicado. Portanto nem Cache em disco nem prévia residente produzem
falso sucesso. A verificação canônica não usa `cfg(test)`, variável de ambiente de teste
ou Host executado no mesmo processo como evidência dessa propriedade.

A mesma verificação exige `exportedAfterReopen=true`, registra os PIDs dos Hosts que
salvaram e reabriram o Projeto e aceita o terminal do Processador somente quando
ele está correlacionado ao segundo Host. A lista observada precisa conter
exatamente as duas tentativas esperadas — JPEG com Cache vazio e falha com
Original ausente —, cada uma com seu terminal único; uma terceira tentativa é
falha mesmo que tenha seu próprio terminal de encerramento. A
preparação também sincroniza o processo auxiliar do perfil `debug` com o binário
executado pelo Tauri e compara os resumos antes de abrir a interface gráfica,
impedindo que uma compilação `release` anterior contamine a prova.

## Fechamento por RED → GREEN

As revisões independentes foram convertidas em regressões públicas antes de
cada correção: arquivo dourado intermediário v2 ausente; referência de WebDriver
perdida quando o encerramento falhava; reimportação duplicada; geometria 1×1
com Original ausente; tentativa adicional de Processador aceita pela prova;
Cache de um vínculo religado recuperado após Undo/Descarte; observação obsoleta
abortando o restante do lote do Monitor; confirmação consumida por demanda/nova
tentativa sem reidratar as dimensões no Projeto; e reidratação no Host sem
atualização da Projeção já mantida pela WebView. Os respectivos testes falharam
primeiro e passaram depois da restauração do arquivo dourado, encerramento
confirmado, reutilização da ocorrência, vínculo opaco do Cache por
`mediaId + path`, adoção independente por ocorrência, reidratação compartilhada
entre Monitor, demanda e nova tentativa e evento causal que recarrega a
Projeção sem criar revisão criativa.

A revisão final acrescentou uma regressão de reabertura sem Cache que entrega a
confirmação estável exatamente durante `project_ui_ready`, mantém a inicialização
pendente e comprova Canvas e Painel sincronizados sem Histórico. A consulta
autoritativa de alvo deixou de transportar `PhotoPlacementMode`; somente a
mutação conserva o modo necessário à geometria. O Monitor removeu o intermediário
genérico e executa diretamente, no executor bloqueante, a observação e a
reidratação do fluxo real. As suítes integrais finais cobrem 233 testes da
interface e 475 testes Rust aprovados, com 16 testes Rust explicitamente
ignorados e roteados às verificações reais Windows/Processador.

O artefato canônico é
`docs/research/artifacts/0037-issue-17-first-photo-composition.json`. O script
de controle captura `HEAD` e todas as fontes antes e depois, exclui apenas esse
destino, exige verificações não vazias e remove temporários, processos e portas
de escuta antes de aceitar `sourceInputsDirty=false`.
