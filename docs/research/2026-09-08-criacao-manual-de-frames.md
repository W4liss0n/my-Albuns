---
status: current
document: research
date: 2026-09-08
---

# Criação manual de Frames

Este recorte da issue #20 implementa `Editar > Adicionar Frame` e o mesmo
comando no menu de contexto da área vazia do Canvas durante o Modo de edição
da Lâmina, conforme o [design da Janela do Projeto](../design/0001-estrutura-da-janela-do-projeto.md).

Cada comando cria imediatamente um placeholder na superfície ativa e seleciona
somente o Frame novo. A proporção é 3:2, com largura de 40% da superfície,
reduzida quando necessário para caber na altura disponível. Uma Lâmina dupla
usa toda a largura e permite atravessar a divisão; uma Página única usa apenas
o lado ativo. Não há tamanho físico fixo nem ferramenta de desenho.

O Frame entra no topo da Pilha visual. Os Frames existentes preservam seus
identificadores, geometrias, Fotos e ordem. O comando não cria nem importa
mídia; adicionar uma Foto posteriormente pode preencher esse placeholder.

## Autoridade e Histórico

`ProjectIntent::AddFrame` recebe a Lâmina e retorna o identificador do Frame
criado. O Core calcula a geometria com a mesma política proporcional já usada
na criação de um Frame preenchido durante a edição. O cliente mantém somente
a seleção temporária e encaminha o comando pela fila compartilhada de mutações.

Uma criação corresponde a uma ação de Undo/Redo. Desfazer remove o Frame e
limpa sua seleção; refazer restaura o conteúdo persistente, sem restaurar a
seleção temporária. Salvamento e reabertura preservam o placeholder e sua
posição na Pilha. Uma Lâmina inválida não altera o Projeto nem descarta Redo.

## Verificação

Os testes públicos do Core cobrem criação, centralização nas três superfícies,
limite de altura, preservação de Frames existentes, identidade, Histórico,
Salvamento, reabertura e preenchimento posterior por uma Foto. A interface é
exercitada pelos dois menus, pela seleção do novo Frame e pela fila com criação
pendente seguida de Salvar e Desfazer, tanto no sucesso quanto na falha.

O corpus `manual-frame-cases.json` é produzido e conferido pelo Core. Os
cenários da aceitação visual usam esse corpus no aplicativo React/Pixi para
verificar menus, clique direito real na Lâmina e no fundo do Canvas, geometria
desenhada e Undo/Redo. A captura é feita em navegador headless; os testes
automatizados com janelas nativas continuam suspensos.

A interação segue o contrato de eventos do
[Pixi 8](https://pixijs.com/8.x/guides/components/events), versão instalada
8.19.0: o alvo interativo distingue Frames e áreas vazias, e a área de captura
do palco inclui o fundo do Canvas. A superfície contextual compartilhada
preserva foco, teclado, fechamento por Esc e consumo do clique externo.

## Escopo restante

A [exclusão no Modo de edição](2026-09-08-exclusao-de-frames.md) continua esta
entrega. A issue #20 permanece aberta para troca de conteúdo, cópia/colagem e
outros comandos. O Core ainda não expõe travamento persistente de Layout; este
recorte não acrescenta esse estado. O Canvas respeita o indicador de Layout
travado quando fornecido, e o comando de criação está disponível no estado
atual de edição com Layout destravado.
