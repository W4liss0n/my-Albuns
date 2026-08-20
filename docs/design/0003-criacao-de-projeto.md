---
status: accepted
document: design
---

# Criação de Projeto

**Referência visual vigente:** [Criação de Projeto](../references/ui-programa-diagramacao/Criar%20projeto.dc.html)

## Objetivo

O fluxo de `Novo Projeto` coleta apenas as definições iniciais do Álbum. Nome e Localização permanecem sob responsabilidade do diálogo nativo do Windows.

## Estrutura

A criação possui exatamente duas etapas dentro do aplicativo:

1. `Configurações`, dedicada à base física e estrutural do Álbum;
2. `Personalização`, dedicada aos padrões visuais iniciais.

`Novo Projeto` aparece como contexto no título da janela e não se repete na
área de conteúdo. A faixa superior compartilhada centraliza as duas etapas no
espaço disponível à esquerda do controle `Modelo inicial` e mantém esse controle
alinhado à direita durante todo o fluxo. Quando recebe foco, o controle composto
usa somente uma mudança neutra de fundo; o campo e a ação de salvar não
desenham contorno azul nem bordas de foco adicionais.

### Configurações

A primeira etapa mantém uma prévia proporcional da Lâmina aberta à esquerda e
organiza os controles à direita nesta ordem:

- Unidade de medida;
- Dimensão da Lâmina fechada;
- Sangria e Área de segurança;
- quantidade inicial de Lâminas;
- Resolução do Projeto;
- Configuração das extremidades.

A largura informada em `Dimensão da Lâmina fechada` corresponde à largura da
Página. A interface deriva a largura da Dimensão da Lâmina aberta multiplicando
esse valor por dois; a altura é a mesma nas duas representações. O núcleo recebe
e valida sempre a Dimensão da Lâmina aberta.

A quantidade começa em 18 Lâminas e os botões do contador alteram o valor em
passos de duas. A interface não inventa um máximo funcional: o núcleo continua
dono da validade da quantidade, inclusive para Álbuns longos. A Unidade interna
`in` é apresentada à pessoa como `pol` em botões, campos e prévias. Medidas em
polegadas usam no máximo três casas decimais, com arredondamento somente da
apresentação; o valor físico interno permanece em micrômetros inteiros.

As medidas, a Sangria do Projeto e a Área de segurança valem para o Álbum
inteiro e podem ser alteradas posteriormente nas Configurações do Projeto. A
etapa não repete essa informação em um card de aviso.

#### Estado temporário das Predefinições

`Predefinição`, apresentada na interface como `Modelo inicial`, é um
**PLACEHOLDER UI** enquanto não existir contrato de
aplicação e persistência no backend. As opções incorporadas e as opções salvas
pelo usuário funcionam somente na sessão atual da tela e guardam, em memória, os
valores de `Configurações` e `Personalização`. Reabrir o aplicativo descarta as
predefinições criadas nessa sessão.

O código que materializa esse comportamento deve permanecer marcado com
`PLACEHOLDER UI` e `data-placeholder-feature="new-project-presets"` até a
substituição por uma porta de aplicação real. A marcação não deve ser removida
apenas porque o fluxo local funciona visualmente.

A linha superior da prévia identifica a dimensão da Lâmina aberta, a quantidade
de Lâminas e as guias técnicas. As medidas e o DPI permanecem nos próprios
controles, sem repetir essas informações em um card de resumo.

```text
┌──────────────────────────────────────────────────────────────────┐
│    1 Configurações ─ 2 Personalização        Modelo inicial [⌄]  │
├───────────────────────────────────────────┬──────────────────────┤
│  Lâmina aberta · quantidade · guias       │  Unidade              │
│          prévia proporcional              │  Dimensão fechada     │
│                                           │  Sangria · Segurança │
│                                           │  Lâminas              │
│                                           │  Resolução do Projeto │
│                                           │  Extremidades         │
├──────────────────────────────────────────────────────────────────┤
│  Cancelar                                           Continuar    │
└──────────────────────────────────────────────────────────────────┘
```

### Validação

`Continuar` valida todos os campos da etapa `Configurações` e só avança quando o conjunto estiver válido. O controle inválido mantém sua indicação visual e o resumo deduplicado permanece como descrição acessível, sem alterar o layout. Quando o formulário passa a ter erros, o Tooltip abre automaticamente no primeiro campo inválido e permanece visível até a correção completa ou um clique fora desse campo. Dispensá-lo mantém as bordas vermelhas. Clicar ou mover o foco do teclado novamente para qualquer campo ainda inválido reabre o resumo ancorado nesse campo; hover sozinho não o reabre. Uma nova ocorrência após o formulário voltar a ficar válido também pode abri-lo. O Tooltip usa os tons claros de erro da paleta compartilhada, sem superfície escura invertida. O foco é transferido para o primeiro campo inválido na ordem visual.

O fluxo não usa um modal genérico para listar erros de preenchimento. Corrigir um campo atualiza seu estado e a prévia imediatamente.

### Personalização

A segunda etapa contém:

- Background padrão;
- Overlay padrão;
- Padrão dos Frames.

Background e Overlay oferecem `Escolher imagem...`, que abre o seletor nativo do Windows. A imagem escolhida permanece provisória e já pode ser exibida na etapa, mas só é vinculada à aba `Decorativos` quando a criação do Projeto for concluída. Cancelar qualquer parte do fluxo não importa nem copia o arquivo.

### Prévia viva

A etapa `Personalização` contém uma reprodução de Lâmina com Frames de demonstração. Ela mostra em conjunto:

- o Background atual;
- o Overlay atual;
- a presença, a cor e a espessura da Borda padrão dos Frames.

A demonstração distribui quatro Frames em duas colunas por Página, com margens e
intervalos uniformes. Cada Frame usa um preenchimento neutro translúcido, sem o
antigo contorno tracejado, para continuar legível sobre o Background configurado.

O bloco `Frames` não usa checkbox ou campo numérico genérico. Ele apresenta a
espessura da Borda padrão em um controle deslizante, onde zero aparece como
`sem borda`, seguido pelas amostras branca, escura e dourada. Um segundo controle
deslizante mantém o `Espaço entre Frames` como medida física transitória em
micrômetros, apresenta sua conversão na Unidade escolhida em `Configurações` e
atualiza imediatamente a reprodução.

A reprodução desenha a Borda padrão integralmente para dentro de cada Frame. A
dimensão externa do Frame permanece inalterada e somente sua área visível interna
é ocupada pela espessura da Borda, conforme a regra geral da Borda do Frame.
Após a criação, o `CompositionCore` materializa as quatro áreas internas de
preenchimento que Editor e Exportação desenham sem recalcular essa regra. A
reprodução transitória anterior à criação espelha o mesmo arranjo apenas para
apresentar o rascunho ainda não submetido ao núcleo.

`Espaço entre Frames` permanece um placeholder visual enquanto não existir no
contrato do Padrão de Frame. Seu estado afeta somente a reprodução atual, não é
enviado ao núcleo nem salvo em Predefinição ou Projeto, e deve permanecer marcado
no código com `PLACEHOLDER UI` e
`data-placeholder-feature="new-project-frame-gap"`.

A reprodução mostra sempre uma Lâmina dupla e mantém a proporção de largura e altura definida na etapa `Configurações`. O formato escolhido para a primeira ou a última Lâmina não desativa lados nessa demonstração, pois sua finalidade é permitir a configuração conjunta dos escopos esquerdo, direito e de Ambos os lados.

As duas etapas reutilizam o mesmo painel de prévia: fundo, cabeçalho, metadados,
legenda, espaçamento, encaixe proporcional, borda, sombra e guias técnicas. Ele limita
a superfície simultaneamente pela largura e pela altura disponíveis e reserva
as faixas externas de cabeçalho e legenda. Somente o conteúdo interno da Lâmina
e os controles espaciais da `Personalização` variam entre as etapas; não existe
uma segunda estrutura visual nem outra regra de dimensionamento.

Qualquer alteração nesses controles atualiza a reprodução imediatamente. As imagens provisórias escolhidas para Background ou Overlay também são compostas na prévia antes da criação do Projeto.

A reprodução divide a própria Lâmina em duas regiões clicáveis, uma para o lado
esquerdo e outra para o lado direito. Clicar em qualquer ponto do painel de
prévia fora da Lâmina, inclusive cabeçalho, metadados e legenda, fixa `Ambos os
lados`; a região central da Lâmina não constitui um terceiro alvo. Cada lado fora
da seleção fixa recebe uma atenuação neutra
translúcida sobre toda a superfície. O hover reduz essa atenuação no escopo
candidato e o cobre com uma tonalidade neutra compartilhada pelo tema, sem
desenhar contorno nem alterar o contraste individual dos Frames. Um lado que já
pertence à seleção fixa nunca recebe hover; quando `Ambos os lados` está fixado,
nenhum lado apresenta pré-seleção. Mover o ponteiro nunca oculta, substitui ou
modifica a seleção fixa.

A seleção usa um contorno azul sólido desenhado integralmente para fora da
superfície do escopo, mantém sua superfície com presença total e seus Frames com
o contraste mais forte. O foco por teclado conserva um contorno azul-claro
pontilhado, recuado da borda, e contraste intermediário nos Frames que não
pertencem à seleção.

A tonalidade de hover fica sobre o conteúdo da Lâmina e abaixo dos indicadores
de foco, da seleção fixa e das guias técnicas. Frames que não pertencem à
seleção nem ao foco permanecem mais discretos.

Os controles de Background e Overlay atuam no escopo fixado. Mover o ponteiro
sem clicar altera somente a pré-seleção transitória da reprodução e não troca a
configuração selecionada.

A reprodução é somente visual: sua Lâmina, seus Frames e qualquer conteúdo demonstrativo não são copiados para o Projeto criado.

### Disposição da etapa

`Personalização` usa duas colunas durante toda a etapa:

- a coluna esquerda, maior, mantém a reprodução da Lâmina;
- a coluna direita contém os controles de Background, Overlay e Padrão dos Frames;
- o rodapé fixo contém `Voltar`, `Cancelar` e `Criar Projeto`.

Os controles podem rolar dentro de sua região quando necessário, sem retirar a reprodução da área visível. O rodapé também permanece acessível independentemente da rolagem.

```text
┌──────────────────────────────────────────────────────────────────┐
│    1 Configurações ─ 2 Personalização        Modelo inicial [⌄]  │
├───────────────────────────────────────────┬──────────────────────┤
│                                           │  Background          │
│                                           │  Overlay             │
│          reprodução da Lâmina             │  Padrão dos Frames   │
│                                           │                      │
│                                           │                      │
├───────────────────────────────────────────┴──────────────────────┤
│  Cancelar                         Voltar         Criar Projeto   │
└──────────────────────────────────────────────────────────────────┘
```

O fluxo permite voltar à etapa anterior sem perder valores. `Cancelar` permanece
isolado à esquerda do rodapé para reduzir encerramentos acidentais; `Voltar`
fica agrupado à direita, imediatamente antes da ação principal `Criar Projeto`.
`Cancelar` encerra a criação sem produzir arquivo. `Criar Projeto` existe na etapa final
e, depois de validar as duas etapas, abre o diálogo nativo do Windows.

## Nome e Localização

O Nome do Projeto e o destino não são campos das etapas do aplicativo. No diálogo nativo, o usuário escolhe a pasta e informa o nome do arquivo.

Cancelar esse diálogo retorna à etapa `Personalização` com todos os valores preservados. Nenhum arquivo, Identidade de Projeto ou entrada em `Projetos recentes` é criado antes da confirmação válida do diálogo nativo.

O destino pode ser local, UNC, unidade mapeada ou caminho verbatim local/UNC. A [política de caminhos](0011-resolucao-e-politica-de-caminhos.md) valida a forma totalmente qualificada e a capacidade de gravação antes de criar o arquivo; falha não deixa Projeto parcial.
