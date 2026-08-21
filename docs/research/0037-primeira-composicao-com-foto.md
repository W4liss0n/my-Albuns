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
reidratados pelo Host, e `baseFillZoom` é calculado pelo `CompositionEngine`.
Isso evita gravar propriedades derivadas e permite que uma mudança estável do
Original atualize a composição sem revisão ou Histórico. O runtime indexa cada
observação por `mediaId + path`; assim, Religação, Undo e Redo não reutilizam as
dimensões observadas para outro vínculo lógico.

O targeting é uma consulta pura do `ProjectCore`: valida a superfície e varre
Frames da frente para trás. O drop envia coordenadas, não uma decisão da
WebView. A mesma função é usada para destaque e mutação; se a resposta visual
ficar obsoleta, a mutação ainda revalida a coordenada atual e falha sem revisão
quando inválida.

O modo normal reutiliza um único primeiro Layout determinístico, enquanto o
Modo de edição cria apenas o retângulo proporcional pedido. Não foi criado um
framework de Layouts, um store de seleção ou uma segunda pilha de Histórico.

## Protocolo de fidelidade e ausência

`scripts/Test-ProductiveJourney.ps1` usa uma Foto JPEG externa real, seleciona
o arquivo em diálogo nativo e aciona o duplo clique pela WebView2. A rodada
confirma schema v3 e vínculo externo sem metadados derivados, materializa a
textura opaca no Canvas, salva, reabre em outro Host e exporta pelo Processador.

A amostra central da Foto é comparada entre captura do Canvas e JPEG final com
tolerância máxima explícita de 32 níveis por canal, cobrindo a recompressão
JPEG. Uma amostra fora do Frame mantém tolerância 8 para o Background. O runner
também verifica o hash e os bytes do Original antes e depois das operações.

Há duas provas complementares de ausência:

1. a jornada confirma primeiro que existe uma representação JPEG no Cache,
   remove o Original antes de uma segunda Exportação e exige mensagem com
   `Religar`/`Religue`, terminal `export_failed` em `source_verification` e
   nenhum arquivo publicado; assim, nem um Cache populado produz falso sucesso;
2. o teste real do Host captura os bindings quando o Original ainda existe,
   remove-o depois e exige `SourceUnavailable` do Processador, com Cache
   explicitamente vazio e fora de todos os caminhos requeridos.

O artefato canônico é
`docs/research/artifacts/0037-issue-17-first-photo-composition.json`. O wrapper
captura `HEAD` e todos os inputs antes e depois, exclui apenas esse destino,
exige checks não vazios e remove scratch, processos e listeners antes de
aceitar `sourceInputsDirty=false`.
