---
status: accepted
document: design
date: 2026-09-15
implementation-readiness: ready-for-agent
---

# Pastas de organização e Projeto v12

## Organização no Projeto

Cada Foto ou Decorativo pertence a no máximo uma Pasta de organização. As pastas
são planas, separadas por aba e pertencem ao Projeto. `Todas` reúne também as
imagens sem pasta. Organizar não move nem renomeia arquivos no disco, não altera
Frames, usos, composição ou Exportação.

Uma pasta possui UUID v4 canônico, nome, tipo e membros identificados pela mídia
do Projeto. A ordem das pastas é a de criação. O nome tem de 1 a 80 caracteres,
sem caracteres de controle; espaços externos são removidos ao criar/renomear.
Nomes iguais sem distinção de caixa na mesma aba são recusados, assim como
`Todas` e `Ausentes`, reservados aos filtros. Fotos e Decorativos podem usar o
mesmo nome de pasta.

Criar, renomear, excluir e mover a seleção são comandos atômicos na fila de
mutações compartilhada. Alterações efetivas marcam o Projeto e cada comando
gera uma ação de Undo/Redo. Repetir a associação existente não cria revisão.
Um destino ou membro inexistente, ou uma seleção de tipos mistos, é recusado
integralmente. Remover uma mídia do Projeto remove também sua associação;
desfazer restaura ambas. Religar e substituir conservam a identidade da mídia
e sua pasta.

Excluir uma pasta é imediato e mantém suas imagens no Projeto, em `Todas`.
`Sem pasta` retira somente a associação dos itens selecionados. Imagens
importadas começam sem pasta; a organização é uma ação explícita posterior.

## Painel de imagens

O `+` existente abre um formulário compacto ancorado para o nome e as ações
`Cancelar`/`Criar`. O chip exibe nome e contagem total da pasta. Clique filtra;
botão direito ou `Shift+F10` abre `Renomear…` e `Excluir pasta`. Renomear usa o
mesmo formulário e `Salvar`. Não há faixa de sucesso, cancelamento ou validação:
erros de nome aparecem no campo e em tooltip somente após tentativa de envio.

O menu das imagens oferece `Mover para pasta…`, aplicado à seleção preservada
pelo clique direito. Um formulário compacto permite escolher uma pasta da aba
ou `Sem pasta`, com `Cancelar`/`Mover`. Esc cancela sem alteração; enquanto uma
ação é enviada, não há submissão duplicada. Os atalhos do álbum não atravessam
esses formulários.

Arrastar uma miniatura até a tag de uma pasta da mesma aba move somente a imagem
arrastada para essa pasta, pelo mesmo comando de organização. O destino recebe
contorno azul durante o gesto; soltar confirma em uma única ação de Histórico,
sem abrir formulário nem mudar o filtro ativo. Esc, perda de foco ou cancelamento
do ponteiro encerram o arrasto sem alterações. `Todas`, `Ausentes` e `+` não são
destinos de organização. O arrasto para a Lâmina conserva seu comportamento.

Durante o arrasto, uma prévia semitransparente da imagem acompanha o ponteiro
em toda a janela, inclusive fora do Painel de imagens. Usa a miniatura já
disponível e mantém a proporção, limitada a 80 × 60 px, com pequeno afastamento
do mouse. Não intercepta os destinos nem troca o cursor sobre as pastas.
Imagens ausentes conservam sua prévia em Cache; sem prévia, usam o placeholder
do painel. Soltar, cancelar, ocultar o painel ou bloquear a interação remove a
prévia imediatamente.

Os chips mantêm o padrão da referência vigente: cores neutras, seleção azul,
contagem discreta, `+` tracejado, sem uma nova linha permanente. Nomes longos
têm elipse e tooltip; a faixa permite rolagem horizontal e acesso por teclado.
Formulários usam controles, espaçamento e superfície flutuante do programa.

A pasta selecionada cruza Busca, Filtro de uso e Ausentes e conserva a ordenação
normal, inclusive para arquivos ausentes. `Todas` limpa pasta e Ausentes.
Cada aba guarda seu filtro de pasta durante a sessão. Excluir ou desfazer a
criação da pasta ativa retorna a `Todas`; refazer não reativa o filtro antigo.
Itens ocultos saem da seleção. O filtro é transitório, sem Histórico nem
Salvamento, e o planejamento de miniaturas considera a associação atualizada.

## Persistência

O envelope v12 conserva os campos v11 e acrescenta `mediaFolders`, obrigatório
inclusive vazio. Cada entrada fechada contém `id`, `kind`, `name` e `mediaIds`.
O codec tem DTO próprio, separado do domínio e do IPC. São recusados campos
desconhecidos, IDs não canônicos/repetidos, nomes inválidos/duplicados,
referências inexistentes, tipos incompatíveis e mídia em mais de uma pasta.

Projetos v1–v11 abrem com pastas vazias; só Salvar explícito grava v12. Salvar
como, Cópia externa, Recuperação e Projeto modelo conservam a organização.
As identidades das pastas e mídias são locais ao Projeto e permanecem válidas
quando a identidade do Projeto muda. As cópias seguem independentes.

## Verificação

O Core é o limite de verificação da associação exclusiva, atomicidade,
Histórico, migração, persistência e independência. Os testes da interface
exercitam filtros combinados, seleção, formulários, cancelamento e envio único;
os da fila verificam comandos adjacentes, falha, Salvamento e Undo. Cenários
visuais usam o painel de produção e a referência vigente.
