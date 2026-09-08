---
status: current
document: research
date: 2026-09-08
ticket: 20
platform: windows
---

# Copiar e colar Frames

No Modo de edição, `Editar > Copiar` e `Ctrl+C` capturam a seleção inteira.
`Editar > Colar` e `Ctrl+V` criam novas ocorrências na Lâmina atualmente isolada,
incluindo Fotos e placeholders. Somente as novas ocorrências ficam selecionadas.
A ordem relativa, a geometria e todos os ajustes persistentes são preservados.

## Autoridade e duração da cópia

O Core mantém o retrato dos Frames na Sessão editável, fora do Documento e do
Histórico. `CopyFrames` valida toda a seleção antes de substituir a cópia; não
muda Revisão, estado de alteração ou ramos de Undo/Redo. A projeção expõe somente
`canPasteFrames`, sem enviar o conteúdo da área de transferência ao frontend.

A cópia permanece válida após editar ou excluir a origem e ao navegar entre
Lâminas. Reabrir ou adotar outra identidade por Salvar como inicia uma área de
transferência vazia. Outra Janela de Projeto tem sua própria Sessão e nunca
recebe a cópia. Não há integração com a área de transferência do Windows.

`PasteFrames` cria novos IDs e acrescenta o grupo ao topo da Pilha visual em
ordem determinística. Na origem, o Canvas informa o desejo de deslocar 16 pixels;
o Core limita esse deslocamento ao espaço disponível para o conjunto inteiro.
Se um eixo não permite avançar, ambos ficam sem deslocamento. Em outra Lâmina
com a mesma superfície, a posição é preservada.

Na Página única, o Core mapeia o conjunto completo proporcionalmente para a
superfície ativa. De Página única para Lâmina dupla, o destino é somente a Página
do mesmo lado lógico. O mapeamento usa bordas inteiras em micrômetros, com um
único arredondamento, e valida a composição inteira antes de criar Histórico.

As Fotos reutilizam os vínculos existentes e seus ajustes de Pan e Zoom. Se um
Undo removeu a importação depois de Copiar, o Core restaura a referência capturada;
uma reimportação do mesmo caminho é reutilizada. Não são copiados Originais ou
Cache. Um vínculo ainda existente mantém a identidade e um eventual relink atual.

## Histórico, interface e verificação

A colagem inteira forma uma ação de Undo/Redo, persiste no Salvamento e na
reabertura e aparece na composição da Exportação. Uma composição de Exportação
congelada anteriormente permanece inalterada. O esquema persistente continua v3.

Copiar, Colar, Salvar e Histórico usam a mesma fila da Janela. A disponibilidade
da cópia é conferida no resultado anterior da fila; um `Ctrl+V` rápido pode esperar
o `Ctrl+C` pendente. Uma falha cancela os comandos adjacentes dependentes. A seleção
das novas ocorrências é publicada junto ao resultado concluído, antes de um Undo
seguinte; mudar de Lâmina durante a espera não seleciona Frames fora do destino.

O catálogo canônico fornece os comandos e atalhos. Campos de texto, Painel de
imagens, menus e diálogos mantêm sua própria interpretação de `Ctrl+C/V`.
Colar funciona sem seleção prévia no destino; repetir automaticamente a tecla
não repete o comando.

Os testes públicos do Core verificam adaptação das superfícies e lados, limite
do deslocamento, IDs e ordem, imutabilidade da cópia, vínculos, isolamento,
Histórico, Salvamento e reabertura. O corpus de aceitação visual é produzido e
conferido por esses testes. Testes do controlador exercitam a fila em sucesso
e falha, a seleção, a navegação e a substituição da Sessão; atalhos são verificados
com os diferentes contextos de foco.

## Continuidade da issue #20

Este recorte usa a geometria e os ajustes que já existem no Core. O travamento
persistente de Layout e estilos individuais de Frame pertencem às entregas
correspondentes. Quando o travamento existir, Copiar continuará disponível e
Colar ficará desabilitado em Layout travado, conforme a especificação aceita.
