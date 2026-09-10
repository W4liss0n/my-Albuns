---
status: accepted
document: design
date: 2026-09-09
updated: 2026-09-10
ticket: 27
implementation-readiness: ready-for-agent
---

# Catálogo global de Layouts personalizados

## Captura e identidade

Salvar disposição como Layout está disponível no Modo de edição, tanto em
Design da Lâmina quanto no menu Editar, inclusive com Frames selecionados.
O comando captura a geometria confirmada no Core depois das edições pendentes.
Exige ao menos um Frame e não altera composição, travamento, Projeto ou Histórico.

A definição contém superfície, escopo e retângulos ordenados. Uma travessia
central determina escopo por Lâmina; sem travessia, o escopo é por Página.
Conforme correção solicitada em 10/09/2026, ambos preservam as coordenadas
exatas, dimensões, intervalos e ordem, inclusive os ajustes manuais. Salvar e
reaplicar na mesma superfície não recentraliza nem desloca os Frames. A
centralização permanece uma regra do Gerador. A captura de Página única usa
a superfície ativa local.

O retorno de salvar ou de detectar uma duplicata aparece junto ao botão em
Design da Lâmina. Quando esse controle não está visível, aparece abaixo do
menu Editar. O aviso compartilha formato, seta, tipografia e sombra com os
balões de validação das entradas, usando as cores neutras do programa e um
botão de fechar. Ele não cobre o canto inferior do Canvas.

A identidade geométrica compara escopo, tipo e proporção da superfície e a
sequência normalizada de retângulos; não depende de DPI, Unidade ou tamanho
físico. Outra ordem representa outro Layout. Fotos, estilos e identidades
de Frames ou de Projetos não entram no catálogo.

Conforme decisão do usuário de 09/09/2026, a deduplicação da apresentação ocorre
por origem. A mesma geometria pode ter uma miniatura em Automáticos e outra em
Personalizados. Último Layout e Favorito compartilham a miniatura da própria
origem, sem fundir as duas seções.

## Persistência e atualização

O Host mantém `Layouts/catalog.json` sob a raiz Roaming do aplicativo. O envelope
fechado v1 contém `schemaVersion`, `revision` e `entries`. Cada entrada possui
UUID v4 canônico e a definição geométrica completa. A ordem do vetor é a ordem
estável de criação; exclusões preservam a ordem dos sobreviventes.

Cada criação ou exclusão lê a versão mais recente sob o mutex entre processos,
incrementa a revisão e publica o documento inteiro por substituição atômica.
A operação de arquivo compartilhada com Preferências pertence a `local_store_io`.
Duplicatas e exclusões já realizadas são idempotentes e não incrementam revisão.

Schema desconhecido, conteúdo inválido, revisão regressiva, leitura ou escrita
malsucedida não apagam a cópia confirmada nem sobrescrevem o arquivo. Arquivo
ausente só representa catálogo inicial quando a instância ainda não confirmou
uma revisão persistida. O erro é apresentado ao usuário.

O catálogo é hidratado no Host ao iniciar e consultado ao montar ou reconectar a
interface, abrir o Painel de Layouts, receber foco ou clicar em Atualizar
Personalizados. Uma revisão nova invalida as consultas preparadas no Core. Uma
falha de atualização conserva as miniaturas anteriores sem habilitar uma
consulta obsoleta. A atualização manual também permite repetir uma consulta.

A sessão do Core conserva o catálogo fora das revisões do Projeto. Undo/Redo
não restaura versões globais antigas. A organização automática recebe essa
fonte explicitamente e mantém a prioridade: Último Layout, Favoritos,
Personalizados, Gerador e reserva. A etapa de Favoritos é implementada pelo
ticket #29.

## Feedback e exclusão

Salvar apresenta um aviso não modal. A duplicata conserva a identidade existente
para localizar e realçar sua miniatura na próxima abertura compatível do Painel.
Uma abertura incompatível não consome essa indicação. Não há nome nem diálogo
na criação.

A lixeira usa o diálogo pertencente à Janela do Projeto, com Cancelar como
fechamento padrão. A confirmação remove exclusivamente a entrada global.
Geometria aplicada, Último Layout e cópias favoritas pertencem ao Projeto e
permanecem válidos. A ausência da entrada global retira a lixeira dessas cópias.

Criação e exclusão participam da fila de operações da Janela, para capturar a
geometria após a edição pendente e preservar a revisão observada por Salvar e
Undo adjacentes. A leitura aguarda operações pendentes, sem publicar uma
projeção ou ocupar a fila de comandos do Projeto.
