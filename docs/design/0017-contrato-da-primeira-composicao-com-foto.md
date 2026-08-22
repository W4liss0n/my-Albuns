---
status: accepted
document: design
date: 2026-08-20
updated: 2026-08-21
ticket: 17-programa-09-primeira-composicao-com-foto
---

# Contrato da primeira composição com Foto

## Objetivo e limite

Este contrato materializa o Programa 09 sobre a composição única do
`ProjectCore`. Ele acrescenta importação JPEG vinculada, Frames preenchidos,
resolução determinística de alvo, enquadramento, persistência e Exportação a partir do
Original. Preserva Identidade, `MediaRef`, resolução e monitoramento de mídia,
Cache, caminhos autorizados, ciclo de vida multiprocesso e a `ExportPipeline`
pública já aceitos.

Não cria `Salvar como`, nova Identidade, configurações globais, recuperação
completa nem comportamento de programas posteriores. Em especial, o ticket 18
continua proprietário de `Salvar como` e bloqueado por este corte.

## Importação e autoridade da mídia

O comando `import_photo` pertence somente à Janela do Projeto. O Host abre o
diálogo nativo `Importar Foto JPEG`, pertencente à Janela do Projeto e limitado
a `.jpg` e `.jpeg`; cancelamento
devolve a Projeção atual e não entra no `ProjectCore`. Uma seleção passa por um
novo `OperationPathContext` e pelo `MediaResolver`, que valida o conteúdo como
JPEG e observa dimensões já ajustadas à orientação declarada. A paleta neutra
usada enquanto a prévia opaca não chega é apenas estado de apresentação do
processo. Extensão ou nome não substituem a validação do fluxo de bytes.

O Projeto persiste apenas `MediaRef { id, kind: photo, path }`. Dimensões,
orientação já aplicada, paleta, disponibilidade, impressão digital, prévia e Cache
são observações transitórias. O Original nunca é regravado pela importação,
composição, Salvamento ou Exportação.

Selecionar novamente o mesmo par canônico `kind + path` não cria outra
ocorrência nem outra revisão: o Core devolve o `mediaId` existente e a WebView
seleciona o respectivo cartão. Essa seleção transitória também não acrescenta
Undo/Redo.

## Modelo persistente v3

`schemaVersion: 3` é a versão pública atual. O DTO fechado conserva o envelope,
documento, Padrões visuais, mídias e Lâminas anteriores e acrescenta a cada
Lâmina uma lista ordenada de Frames:

```text
FrameV3 = {
  id,
  rect: { x, y, width, height },
  photo: null | {
    mediaId,
    transform: { panX, panY, userZoom }
  }
}
```

Retângulos usam micrômetros, têm dimensões positivas e permanecem dentro da
superfície ativa da Lâmina. `mediaId` precisa referenciar uma Foto do catálogo.
Pan é normalizado e `userZoom` é relativo: `1.0` significa nenhum ajuste do
usuário. O Zoom base mínimo que preenche o Frame é derivado das dimensões
observadas e da geometria; não é persistido nem confundido com `userZoom`.

O leitor executa a cadeia pura `v1 -> v2 -> v3`; as duas migrações apenas
acrescentam os valores padrão definidos por suas versões. Abrir não regrava. Um
`Salvar` autorizado publica v3 pelo protocolo atômico existente, sem mudar a
Identidade. Versões futuras, campos desconhecidos, Frames inválidos e
referências quebradas falham fechados.

## Inserção, resolução de alvo e ordem

`AddPhoto` implementa o duplo clique. Primeiro escolhe entre os placeholders da
Lâmina centralizada aquele com menor esquerda e, no empate, menor topo. Se não
há placeholder, cria um Frame no modo atual. `DropPhoto` resolve novamente no
domínio a coordenada recebida; a WebView nunca escolhe um Frame por conta
própria.

Dentro da Lâmina, o alvo é o último Frame da ordem de empilhamento cujo
retângulo contém o ponteiro. Conteúdo, transparência e Opacidade não participam
do teste de acerto. Assim, apenas um Frame superior é substituído. Fora de todos os
Frames, o alvo é a própria Lâmina; fora da superfície, é inválido.

No Modo de edição, uma soltura sobre a Lâmina cria um Frame 3:2 com 40% da largura
da superfície, limitado apenas pela altura disponível. O Frame é centralizado
no ponteiro e deslocado integralmente para dentro quando toca uma borda, sem
redução. No modo normal, o primeiro Layout compatível reorganiza a coleção em
uma grade determinística com margem uniforme. O contrato não cria um catálogo
paralelo de Layouts.

Durante o arraste, o Canvas apresenta somente o contorno azul do Frame ou da
Lâmina resolvida. `Esc`, alvo inválido, saída do Canvas ou soltura externa limpam o
estado transitório. O cartão configura uma imagem de arraste transparente, e
cada nova consulta remove o destaque anterior até que mídia, coordenada,
geração da consulta e alvo resolvido voltem a coincidir. Uma resposta atrasada
ou uma soltura durante a consulta não avança a revisão do Projeto. Inserção,
preenchimento ou substituição retornam
`affectedFrameId`; a WebView seleciona somente esse Frame e atualiza o Painel
contextual depois da Projeção autoritativa. Seleção e destaque não são estado
criativo e não acrescentam Histórico.

## Enquadramento e renderização

O `CompositionEngine` calcula `baseFillZoom` para cobrir integralmente o Frame,
sem área vazada, e compõe a transformação final com `userZoom` e Pan. Fora do
Modo de edição, `Alt+arraste` altera somente Pan e `Alt+roda` altera somente
Zoom. A prévia é transitória; o término do gesto envia um único
`TransformPhoto`, que participa de Undo/Redo. No Modo de edição esses gestos
são ignorados para que a geometria do Frame permaneça sob o modo proprietário.

Canvas e Exportação consomem a mesma Projeção composta. Frame, vínculo, Pan e
Zoom sobrevivem a Salvamento e reabertura; o Host reidrata apenas os metadados
transitórios do Original. Essa inspeção começa pelo Monitor somente depois de
`host_ready` e da WebView registrar o observador de `myalbuns://linked-media-changed`.
A confirmação `project_ui_ready` forma a barreira causal e a transição conjunta
concede uma única inicialização do Monitor, fora do fluxo de execução que inicializa janela
e registro operacional; a resposta atualiza a Projeção pela geração de execução
mais nova, sem criar Histórico. Cada
observação transitória fica associada ao par
`mediaId + path` que a originou, de modo que Undo/Redo de uma Religação restaura
imediatamente as dimensões corretas de cada vínculo.

Antes da primeira Projeção do Host reaberto, o namespace reservado recupera
somente gerações de Cache cujo índice, vínculo opaco `mediaId + path`, caminho
do artefato, tamanho, codec, dimensões e perfil sRGB tenham sido verificados.
As dimensões já orientadas
dessas representações podem reidratar a geometria contextual quando o Original
está ausente, sem nova revisão e sem dupla aplicação de EXIF. Qualquer geração
indexada inválida descarta o índice e as gerações descartáveis do namespace;
em nenhum caso essa reidratação autoriza Exportação.

## Exportação e falha do Original

O Host congela a Lâmina e enumera as referências exatas. O plano público da
Exportação captura o Destino e os Originais, nunca caminhos de Cache. O
Processador decodifica a Foto original para a composição final; uma
representação reduzida pode servir ao Canvas, mas não autoriza nem substitui a
fonte final.

Se um Original não puder ser aberto, a Exportação falha antes da publicação ou
com `SourceUnavailable`, não cria um JPEG de sucesso e orienta a pessoa a usar
`Religar` no Painel de imagens. Cache vazio deve produzir o mesmo JPEG que um
Canvas equivalente dentro da tolerância de codec; Cache antigo não transforma
Original ausente em sucesso.

## Provas públicas

Os testes do `ProjectCore` cobrem v3, migrações, retângulos, ordem de
placeholders, empilhamento, Histórico e ciclo completo de persistência. Testes da WebView cobrem
diálogo e portas, destaque, cancelamentos, seleção e gestos. A jornada produtiva
dirige Global → Host → diálogo nativo → WebView2/Canvas → Processador → Salvar
e reabrir → Exportação, mede Canvas/JPEG com tolerância explícita e repete a
Exportação sem o Original. Antes da primeira Exportação, a verificação observa uma
prévia real, esvazia somente o namespace de Cache isolado da própria jornada
enquanto o diálogo nativo ainda está aberto e mede zero entradas e zero bytes
antes e depois do Processador. A textura residente permanece no Canvas; sua
presença não mascara a falta do Original na segunda tentativa. A limpeza segura valida
a raiz física e cada ancestral imediatamente antes da remoção, recusando
junções e pontos de nova análise; testes Windows com sentinela externa cobrem a raiz e
um ancestral. A tentativa produtiva registra que o Processador pertence ao Host
reaberto, distinto daquele que salvou o Projeto, e exige igualdade exata entre
as duas tentativas esperadas — sucesso com Cache vazio e falha com Original
ausente — e seus terminais correlacionados. Qualquer Processador adicional é
rejeitado mesmo que também tenha encerrado.
