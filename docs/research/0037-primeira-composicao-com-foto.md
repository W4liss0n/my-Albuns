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

- o diálogo do Tauri pode ser configurado no Host com owner, título, filtro e
  callback, mantendo o pathname fora da WebView;
- no PixiJS 8, `eventMode = "static"` e `hitArea` retangular fornecem a
  superfície explícita de interação, enquanto a escolha autoritativa do alvo
  continua no domínio;
- o crate `image` identifica o formato pelo stream e expõe dimensões e
  orientação; o Host troca os eixos observados quando necessário, enquanto o
  Processador continua proprietário da orientação efetiva dos pixels.

Nenhuma dependência nova foi necessária. O codec JPEG, o diálogo, o Canvas e o
protocolo do Processador já pertenciam à árvore aceita.

## Corte adotado

O menor seam persistente é `Frame + PhotoTransform`. Metadados do arquivo são
reidratados pelo Monitor depois de `host_ready`, em trabalho bloqueante fora da
thread de inicialização, e `baseFillZoom` é calculado pelo `CompositionEngine`.
Isso evita gravar propriedades derivadas e permite que uma mudança estável do
Original atualize a composição sem revisão ou Histórico. O runtime indexa cada
observação por `mediaId + path`; assim, Religação, Undo e Redo não reutilizam as
dimensões observadas para outro vínculo lógico.

O targeting é uma consulta pura do `ProjectCore`: valida a superfície e varre
Frames da frente para trás. O drop envia coordenadas, não uma decisão da
WebView. A mesma função é usada para destaque e mutação; a WebView só aceita um
drop quando mídia, ponto e geração coincidem com o destaque visível. Respostas
de projeção do Monitor também têm geração monotônica, inclusive quando a revisão
criativa não muda.

O modo normal reutiliza um único primeiro Layout determinístico, enquanto o
Modo de edição cria apenas o retângulo proporcional pedido. Não foi criado um
framework de Layouts, um store de seleção ou uma segunda pilha de Histórico.

Selecionar novamente o mesmo JPEG é tratado no seam público de importação: o
Core encontra a ocorrência pelo par `kind + path`, atualiza somente os
metadados observados e devolve o `mediaId` existente. A WebView seleciona esse
cartão sem revisão criativa ou entrada adicional de Undo/Redo.

Na reabertura, o namespace de Cache reservado valida índice e artefatos reais
antes de expor qualquer contexto. Somente uma geração cujo caminho, tamanho,
codec, dimensões, perfil sRGB e decode foram confirmados pode reidratar as
dimensões já orientadas da Foto antes da primeira Projeção. Isso evita o
fallback geométrico 1×1 quando o Original já está ausente, sem transformar o
Cache em autoridade para Exportação; índice ou geração corrompidos são
descartados como estado derivado.

## Protocolo de fidelidade e ausência

`scripts/Test-ProductiveJourney.ps1` usa uma Foto JPEG externa real, seleciona
o arquivo em diálogo nativo, seleciona novamente o mesmo arquivo e aciona o
duplo clique pela WebView2. A rodada confirma que a segunda seleção reutiliza o
cartão sem revisão, schema v3 e vínculo externo único sem metadados derivados,
materializa a textura opaca no Canvas, salva, reabre em outro Host e exporta
pelo Processador.

A amostra central da Foto é comparada entre captura do Canvas e JPEG final com
tolerância máxima explícita de 32 níveis por canal, cobrindo a recompressão
JPEG. Uma amostra fora do Frame mantém tolerância 8 para o Background. O runner
também verifica o hash e os bytes do Original antes e depois das operações.

A jornada confirma primeiro uma representação JPEG real no namespace de Cache
isolado do processo. Enquanto o diálogo nativo da primeira Exportação ainda está
aberto, remove somente esse conteúdo derivado por uma travessia fail-closed e
mede zero entradas e zero bytes. O aplicativo real exporta pelo Original; o
namespace continua com zero entradas e zero bytes depois do Processador. Antes
de remover qualquer entrada, o runner verifica por caminho real e atributos que
o root e todos os seus ancestrais não são junctions/reparse points. Dois testes
Windows criam junctions reais no root e em um ancestral e provam que uma
sentinela externa permanece intacta.

Em seguida, a textura já residente continua visível no Canvas, mas o runner
remove o Original antes da segunda Exportação e exige mensagem com
`Religar`/`Religue`, terminal `export_failed` em `source_verification` e nenhum
arquivo publicado. Portanto nem Cache em disco nem prévia residente produzem
falso sucesso. O gate canônico não usa `cfg(test)`, variável de ambiente de teste
ou Host in-process como evidência dessa propriedade.

O mesmo gate exige `exportedAfterReopen=true`, registra os PIDs dos Hosts que
salvaram e reabriram o Projeto e aceita o terminal do Processador somente quando
ele está correlacionado ao segundo Host. A lista de tentativas de Exportação
observada precisa ser exatamente igual à lista correlacionada; uma tentativa
extra é falha mesmo que tenha seu próprio terminal de encerramento. A
preparação também sincroniza o sidecar do perfil debug com o binário executado
pelo Tauri e compara os hashes antes de abrir a GUI, impedindo que um build
release anterior contamine a prova.

## Fechamento por RED → GREEN

As revisões independentes foram convertidas em regressões públicas antes de
cada correção: golden intermediário v2 ausente; referência de WebDriver perdida
quando o teardown falhava; reimportação duplicada; geometria 1×1 com Original
ausente; e tentativa adicional de Processador aceita pela prova. Os respectivos
testes falharam primeiro e passaram depois da restauração do golden, teardown
confirmado, reutilização da ocorrência, reidratação de Cache verificado e
comparação exata das tentativas. As suítes integrais finais cobrem 231 testes de
frontend e 469 testes Rust aprovados, com 16 testes Rust explicitamente
ignorados e roteados aos gates reais Windows/Processador.

O artefato canônico é
`docs/research/artifacts/0037-issue-17-first-photo-composition.json`. O wrapper
captura `HEAD` e todos os inputs antes e depois, exclui apenas esse destino,
exige checks não vazios e remove scratch, processos e listeners antes de
aceitar `sourceInputsDirty=false`.
