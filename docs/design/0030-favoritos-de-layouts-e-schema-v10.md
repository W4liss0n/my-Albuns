---
status: accepted
document: design
date: 2026-09-09
ticket: 29
implementation-readiness: ready-for-agent
---

# Favoritos de Layouts e Projeto v10

## Cópia pertencente ao Projeto

A estrela em Automáticos e Personalizados copia a definição completa, com
superfície, escopo, posições ordenadas e origem. A cópia recebe UUID v4 canônico
próprio e uma ordem persistida. Não contém referência obrigatória ao catálogo
global, Fotos, estilos ou identidades de Frames.

O Core confirma a seleção da consulta preparada antes de alternar a estrela.
Uma consulta vencida não pode favoritar outro candidato. A operação produz uma
única revisão do Projeto e uma ação de Undo/Redo, sem aplicar geometria, criar
posições, mudar o Último Layout ou alterar o travamento. O estado pendente de
salvamento acompanha a alteração. Desfavoritar é imediato e sem confirmação.

A interface apresenta estrela preenchida nas cópias favoritas. A estrela do
Layout atualmente travado continua disponível; as outras miniaturas continuam
indisponíveis conforme o comportamento do painel travado. A fila de comandos
compartilhada conserva a seleção capturada enquanto operações anteriores
terminam, e o Core rejeita uma seleção que se tornou obsoleta.

## Identidade, origem e ordem

A identidade geométrica é a mesma do design 0026 e a deduplicação é por origem,
conforme a decisão registrada no design 0029. Uma geometria presente nas duas
seções pode ter estrelas independentes. Favoritar conserva a seção de origem.

A apresentação lista o Último Layout compatível, os Favoritos, os Personalizados
globais e o Gerador; cada seção preserva essa ordem. Favoritos usam primeiro
`order` e, em empate, seu UUID. A criação recebe a próxima ordem disponível e a
exclusão conserva as ordens existentes. Undo/Redo restaura identidade e ordem.
A aplicação automática mantém Último Layout, primeiro Favorito compatível,
primeiro Personalizado, Gerador e reserva como sequência de prioridade.

A cópia favorita continua disponível quando a origem global muda ou desaparece.
A lixeira só aparece quando existe uma entrada global da mesma geometria.
Desfavoritar uma cópia sem origem remove sua miniatura, exceto na Lâmina que
conserva essa mesma geometria e origem como Último Layout. Alterar parâmetros
do Gerador também não modifica a definição já copiada.

## Arquivo v10

O envelope v10 conserva documento, estilos, mídia, configurações de Layout e
Lâminas do v9 e acrescenta `project.favoriteLayouts`. O vetor é obrigatório,
inclusive quando vazio. Cada elemento contém `id`, `order` e `layout`; a
geometria usa a representação fechada introduzida no v8.

O codec rejeita campos desconhecidos, identidades não canônicas ou repetidas,
ordens fora do intervalo inteiro seguro e definições inválidas ou duplicadas
na mesma origem. Ordens iguais são permitidas porque o UUID resolve o empate.
O vetor não depende da ordem em que foi materializado para ordenar o painel.

Arquivos v1 a v9 abrem com Favoritos vazios. A migração ocorre em memória; apenas
o salvamento explícito publica v10. Salvar como e Cópia externa preservam as
cópias favoritas e suas identidades, mesmo quando a identidade do Projeto muda.
Favoritos de projetos abertos são estados independentes.

## Verificação

Os testes públicos do Core cobrem estrela, consultas vencidas, Undo/Redo,
cópias de posições adicionais sem aplicação, ordenação e desempate, edição e
exclusão da origem, mudança do Gerador, migração, corrupção e portabilidade.
O corpus visual é produzido pelo Core e reproduz estrelas nas duas origens,
cópia sem origem e remoção dessa cópia nas escalas usuais da interface.
