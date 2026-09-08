---
status: current
document: research
date: 2026-09-08
ticket: 20
platform: windows
---

# Exclusão de Frames no Modo de edição

Este recorte continua a criação manual de Frames da issue #20. No Modo de
edição da Lâmina, `Delete` e `Excluir` no menu contextual removem a seleção
inteira sem confirmação. Fotos e placeholders participam da mesma operação.
Os Frames restantes conservam geometria, ajustes e ordem relativa, sem
aplicação automática de Layout. A Lâmina continua existindo mesmo quando
todos os seus Frames são excluídos.

A exclusão retira as ocorrências da composição. As Fotos importadas continuam
no Painel e os Arquivos originais permanecem intactos; o contador de uso é
recalculado e uma Foto sem uso pode ser adicionada novamente.

## Autoridade e Histórico

`ProjectIntent::DeleteFrames` recebe os identificadores capturados na seleção.
O Core valida que existem, são distintos e pertencem à mesma Lâmina antes de
remover qualquer elemento. A validação da seleção é compartilhada com a
ordenação da Pilha visual. Seleções inválidas não alteram o Projeto nem
descartam o ramo de Redo.

Cada exclusão forma uma única ação de Undo/Redo. A fila compartilhada mantém
Salvar e Desfazer atrás de uma exclusão pendente e usa a Revisão resultante.
Falhar na exclusão cancela esses comandos adjacentes dependentes.

A navegação reconcilia a seleção a cada projeção concluída, antes de publicá-la
ao React. Assim, excluir e desfazer rapidamente não conserva por acidente os
IDs removidos quando as atualizações são apresentadas juntas. IDs ainda
existentes permanecem selecionados; restaurar Frames não os seleciona.

Salvar e reabrir preservam o resultado. A composição congelada da Exportação
usa a mesma lista resultante e uma captura anterior permanece inalterada.
O esquema do Arquivo de Projeto continua na versão 3.

## Interface e verificação

O catálogo canônico possui `delete-frames` e o atalho `Delete`. A interface
encaminha o comando somente com seleção no Modo de edição, respeitando campos
de texto, foco no Painel de imagens, menus, diálogos e operações que bloqueiam
a interação. Repetições automáticas da tecla não repetem a exclusão.

Os testes públicos do Core cobrem seleção mista, exclusão individual e total,
preservação de mídia, rejeição atômica, Histórico, Salvamento, reabertura,
composição congelada e o alvo de soltura após remover o Frame superior. Os
testes do Workspace e do controlador exercitam os dois caminhos de comando,
proteção de foco e a fila com sucesso ou falha, incluindo seleção alterada
durante a espera e Desfazer adjacente.

O corpus `frame-deletion-cases.json` é produzido e conferido pelo Core. Os
cenários declarados de aceitação visual usam esses resultados nos componentes
React/Pixi, com clique direito e `Delete` enviados pelo WebDriver. A revisão
inclui também os menus contextuais de ordenação afetados pela nova opção.

O tratamento dos listeners segue o contrato de efeitos do React 19.2.8
(documentação indexada da versão 19.2.7). A tecla especial `Delete` usa o
código definido pelo [WebDriver](https://www.w3.org/TR/webdriver2/#keyboard-actions).
As evidências de execução e os julgamentos visuais ficam fora do controle
de versão. Testes automatizados com janelas nativas permanecem suspensos.

## Escopo restante

A issue #20 continua aberta. Este recorte cobre o estado destravado atualmente
exposto pelo Core. A regra de remover somente Fotos e preservar placeholders
em Layout travado acompanha a futura entrega do travamento persistente.
Exclusão no modo normal com reaplicação de Layout, troca de conteúdo e
cópia/colagem permanecem para os próximos recortes.
