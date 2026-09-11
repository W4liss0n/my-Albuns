---
status: accepted
document: design
date: 2026-09-11
updated: 2026-09-11
---

# Snap de Frames

Este contrato define o encaixe assistido durante o movimento e o redimensionamento
de Frames, conforme as decisões aprovadas em 11/09/2026. O aceite registra o
comportamento a implementar; a calibração e as evidências no aplicativo pertencem
à validação da entrega.

As regras existentes de edição pertencem à
[especificação do produto](../specs/programa-de-diagramacao-de-albuns.md#frames-e-fotos)
e à [estrutura da Janela do Projeto](0001-estrutura-da-janela-do-projeto.md).
O termo Snap de Frames está definido no [glossário](../../CONTEXT.md).

## Referências de alinhamento

Na definição de 11/09/2026, foram incluídas as seguintes referências:

- Bordas e centros de outros Frames, inclusive placeholders.
- Bordas e eixos centrais horizontal e vertical de cada Página ativa.
- Divisão central e centro da composição inteira em Lâmina dupla.
- Linha de corte e limites da Área de segurança.

As referências técnicas correspondem às medidas já definidas no Projeto.
A linha de corte delimita a Área de corte; a Sangria do Projeto é a faixa
entre essa linha e a borda externa. A Área de segurança fornece seus próprios
limites internos de referência.

Exemplo: ao aproximar a borda esquerda de um Frame da borda esquerda de outro,
o encaixe alinha as duas bordas. Uma guia indica o alinhamento durante o gesto.

## Snap de dimensão

O redimensionamento também oferece encaixe por igualdade de dimensão com
outro Frame: largura com largura e altura com altura. Essa correspondência
independe da posição dos Frames; suas bordas não precisam estar alinhadas.
O Frame usado como referência conserva sua geometria.

- Ao redimensionar pela lateral direita ou esquerda, a largura pode encaixar
  na largura de outro Frame.
- Ao redimensionar pela lateral superior ou inferior, a altura pode encaixar
  na altura de outro Frame.

Exemplo: um Frame de referência tem largura de `100 mm`. Ao aproximar a
largura do Frame editado desse valor durante o arraste de sua lateral direita,
o snap permite atingir exatamente `100 mm`, conservando a âncora do gesto.
O mesmo comportamento se aplica à igualdade de alturas.

Nas alças de canto, largura e altura podem encaixar independentemente.
Com `Shift` pressionado, o encaixe em uma dimensão ajusta a outra junto,
preservando a proporção da Caixa delimitadora. Os limites da superfície e
o tamanho mínimo continuam prevalecendo sobre qualquer encaixe.

A escolha entre alvos concorrentes, inclusive com proporção preservada,
segue as regras de sensibilidade e estabilidade deste contrato.

## Seleção múltipla como uma unidade

A Seleção de Frames é tratada como um único objeto para os snaps. Com um Frame,
a referência manipulada é seu retângulo externo; com vários, é somente a Caixa
delimitadora do conjunto.

- Alinhamento usa as bordas e os centros da Caixa delimitadora.
- Igualdade de dimensão compara a largura e a altura totais da Caixa com a
  largura e a altura de Frames externos à seleção.
- Espaçamento usa os limites externos da Caixa em relação aos Frames que
  estão fora da seleção.
- Bordas, centros, dimensões e intervalos dos Frames internos não produzem
  encaixes individuais. Frames selecionados também não são alvos do próprio
  gesto.
- O grupo inteiro acompanha o encaixe: movimento preserva suas distâncias
  relativas e redimensionamento escala posições e dimensões coletivamente.

Essa escolha mantém o gesto coerente com a manipulação do conjunto como uma
unidade e evita que um elemento interno atraia o grupo para um encaixe inesperado.

## Snap de espaçamento

O encaixe de espaçamento contempla duas referências, na horizontal e na vertical:

- **Espaçamento igual:** repetir uma distância existente entre Frames. Por
  exemplo, se o espaço entre o primeiro e o segundo é `20 mm`, o terceiro
  pode encaixar a `20 mm` do segundo durante seu movimento.
- **Espaçamento padrão do Projeto:** usar a distância configurada para o
  Projeto como referência de encaixe entre Frames, mesmo sem existir outro
  par com essa distância na composição.

Os dois tipos de espaçamento funcionam durante movimento e redimensionamento.
Ao redimensionar, a borda manipulada pode encaixar quando sua distância ao Frame
vizinho atingir a medida de referência. As regras das alças, das âncoras e dos
modificadores continuam valendo.

Esses encaixes ajustam a seleção manipulada; os Frames usados como referência
permanecem nas posições confirmadas. A seção de vizinhança detalha a escolha
das referências.

## Espaçamento padrão compartilhado

O Projeto terá uma única configuração de `Espaço entre Frames`, compartilhada
pelos snaps de espaçamento padrão e pela geração de Layouts. O controle da
`Personalização` de Novo Projeto definirá esse valor na criação; o controle de
`Design do Álbum` permitirá alterá-lo posteriormente.

A alteração confirmada orienta os próximos encaixes e consultas de geração,
preservando as composições já existentes. As definições de Layouts já salvas
conservam suas geometrias. A configuração pertence ao Projeto e deve sobreviver
ao Salvamento e à reabertura, sem uma preferência de espaçamento exclusiva dos
snaps.

Para conservar o contrato vigente dos Layouts, novos Projetos começam com
`5 mm`, e Projetos existentes mantêm o intervalo já salvo. O `6 mm` transitório
da prévia atual não substitui a medida persistida. O controle continua usando
a Unidade de medida do Projeto.

Em `Design do Álbum`, a medida integra o draft do formulário e habilita seu
`Aplicar` quando houver alteração válida. Confirmar aplica o espaçamento e os
demais ajustes daquele draft em uma única ação de Undo/Redo. Antes da confirmação,
somente a miniatura do formulário usa o espaçamento pendente; os snaps e o Gerador
continuam usando a medida confirmada. Aplicar não salva o arquivo automaticamente.

A integração reutiliza o intervalo já persistido em `layoutSettings.gapUm`.
O Espaço entre Frames não pertence ao Estilo do Frame e não cria uma propriedade
herdada em cada ocorrência. Restaurar a Borda de um Frame não altera esse valor
do Projeto.

### Situação anterior à integração

A interface existente apresenta `Espaço entre Frames` na `Personalização` de
Novo Projeto e em `Design do Álbum`. Atualmente esse controle altera somente
a prévia, com valor inicial de `6 mm`, sem persistência. Isso está explícito
na [criação de Projeto](0003-criacao-de-projeto.md#prévia-viva) e nos controles
de `PersonalizationStep.tsx` e `AlbumDesignForm.tsx`.

O Projeto já conserva separadamente o intervalo da geração de Layouts, com
valor inicial de `5 mm`, conforme a
[integração dos Layouts](0027-integracao-dos-layouts-e-schema-v8.md).

## Ativação

- Snaps ficam ativos automaticamente durante os gestos de geometria no Modo
  de edição. Não existe botão de ímã nem preferência de ativação permanente.
- Manter `Ctrl` pressionado durante um arraste de geometria suspende
  temporariamente o snap. Soltar a tecla reativa o encaixe durante o mesmo
  gesto, respeitando as tolerâncias e os limites de geometria.
- `Ctrl` + clique continua alternando a Seleção de Frames conforme a regra
  existente. A suspensão pertence ao gesto de arraste.
- `Shift`, `Alt` e sua combinação mantêm as funções atuais de redimensionamento.
- A suspensão temporária abrange alinhamento, igualdade de dimensão e
  espaçamento.

## Regras existentes a preservar

- A edição de geometria ocorre no Modo de edição da Lâmina e exige Layout
  destravado.
- A superfície válida é a Lâmina inteira quando dupla e somente a Página
  ativa quando única.
- Movimento coletivo preserva as distâncias relativas dos Frames;
  redimensionamento coletivo preserva as relações definidas pela Caixa
  delimitadora.
- Os limites da superfície, o tamanho mínimo e as âncoras continuam válidos.
- `Shift` preserva a proporção em alças de canto; `Alt` redimensiona a partir
  do centro; `Shift + Alt` combina os dois.
- Um gesto completo constitui uma única ação de Undo/Redo.

## Comportamento do gesto

### Sensibilidade e estabilidade

- Adquirir um encaixe quando a correção necessária ao arraste ou à alça for de
  até `6 px` lógicos na tela. Essa distância permanece constante na tela,
  independentemente do Zoom do Canvas, da Unidade do Projeto ou do DPI de saída.
- Conservar o alvo adquirido até o movimento livre exigir mais de `10 px` para
  permanecer nele. A distância de soltura maior que a de aquisição evita
  alternância rápida entre posições próximas.
- Medir a aproximação a partir do gesto livre que o ponteiro produziria. O
  próprio resultado já encaixado não pode impedir a saída do snap.
- Escolher primeiro o alvo válido que exige a menor correção. Enquanto um alvo
  estiver retido, um novo candidato próximo não o substitui somente por estar
  momentaneamente mais perto.
- Em empate de correção entre resultados diferentes, preferir alinhamento,
  igualdade de dimensão, espaçamento padrão e espaçamento igual, nessa ordem.
  O desempate restante segue a ordem geométrica das referências na Lâmina,
  da esquerda para a direita e de cima para baixo, sem depender do tempo.
- Referências que resultam na mesma geometria são consolidadas em um único
  encaixe. O indicador pode identificar as correspondências simultâneas sem
  repetir linhas sobrepostas.
- Dimensões que diferem até `1 µm` formam uma única referência de snap no mesmo
  eixo. Agrupar as medidas em ordem crescente, com amplitude máxima de `1 µm`
  por grupo, sem encadear diferenças sucessivas. Entre as referências alcançáveis
  do grupo, escolher pela ordem geométrica acima; a posição do ponteiro e a Pilha
  visual não mudam essa escolha. O encaixe copia a dimensão exata da referência,
  preservando a precisão interna e as restrições do gesto. Uma dimensão que não
  possa ser alcançada exatamente não recebe indicação de igualdade.

Os valores `6 px` e `10 px` são o ponto inicial da calibração no Canvas real.
A validação precisa verificar tanto precisão quanto facilidade de sair do snap.

### Eixos, âncoras e modificadores

No movimento, os eixos horizontal e vertical podem encaixar independentemente.
Nas alças laterais, somente o eixo já controlado pela alça recebe correção.
Nas alças de canto sem preservação de proporção, os dois eixos podem encaixar.

Com `Alt`, a correção respeita o centro fixo e move a borda oposta simetricamente.
Uma borda que permaneça fixa pela âncora do gesto não deve gerar um candidato
que prenda o redimensionamento em sua geometria inicial.

Com `Shift` em um canto, avaliar os candidatos como escalas proporcionais do
conjunto. Se encaixar largura e altura exigir escalas incompatíveis, escolher
somente a que exige a menor correção da alça, aplicando o mesmo desempate
descrito acima. Mostrar os dois encaixes somente quando forem compatíveis com
uma mesma escala. `Shift + Alt` combina essa regra com o centro fixo.

Limites da superfície, tamanho mínimo, não inversão e âncoras prevalecem.
Um candidato incompatível é descartado, e sua guia não aparece como se o encaixe
tivesse sido alcançado. O resultado respeita a quantização física existente.

Pressionar `Ctrl` remove a correção e as guias de snap imediatamente. Ao soltar,
recalcular a partir do ponteiro atual, adquirindo somente alvos dentro da
tolerância. Mudanças de `Shift` ou `Alt` também recalculam os candidatos válidos.

### Referências elegíveis

- Considerar somente a Lâmina isolada e suas Páginas ativas. Nenhum alvo pertence
  a um Lado inativo ou a outra Lâmina.
- Frames externos à seleção oferecem sua geometria, inclusive quando são
  placeholders ou possuem Opacidade reduzida ou zero. Conteúdo da Foto e
  espessura da Borda não alteram o retângulo usado como referência.
- Bordas e centros podem se corresponder no mesmo eixo, como a borda de um
  Frame alinhada ao centro de outro.
- Linha de corte e limites de segurança usam as mesmas posições das guias
  técnicas vigentes. Uma guia técnica desativada por valor zero não cria uma
  referência adicional; bordas da Página continuam elegíveis por si mesmas.
- Referências coincidentes, como divisão central e centro horizontal da Lâmina,
  não geram atração adicional nem desenho duplicado.
- Durante redimensionamento, igualdade de dimensão usa largura com largura e
  altura com altura. Não comparar largura com altura nem aplicar escala
  automaticamente em um gesto de movimento.

### Vizinhança e medidas de espaçamento

Medir o espaço livre entre os retângulos externos, sem somar a espessura da
Borda. Na seleção múltipla, o retângulo manipulado continua sendo somente a
Caixa delimitadora.

Uma relação horizontal exige vizinhos à esquerda ou à direita com alguma
sobreposição vertical. Uma relação vertical exige vizinhos acima ou abaixo com
alguma sobreposição horizontal. Considerar os vizinhos mais próximos em cada
direção, sem pular um Frame intermediário na faixa de comparação.

O espaçamento padrão pode ser atingido em qualquer uma dessas direções. Para
espaçamento igual, usar intervalos existentes entre pares de Frames vizinhos
externos à seleção, no mesmo eixo. Também permitir equilibrar os dois espaços
quando a seleção estiver entre dois vizinhos. Em todos os casos, somente a
seleção manipulada muda; não redistribuir os demais Frames.

Espaços negativos por sobreposição não são medidas de referência. Espaço zero
é válido e coincide com o encontro entre bordas; consolidar esse resultado
com o snap de alinhamento correspondente.

### Indicação visual

- Usar guias finas de `1 px`, constantes na tela, em magenta para distinguir
  os snaps dos contornos azuis de seleção e das guias técnicas vermelha e azul.
- No alinhamento, ligar visualmente as referências que coincidiram.
- Na igualdade de dimensão, indicar a dimensão do conjunto manipulado e a
  dimensão de referência, com a mesma medida na Unidade do Projeto.
- Nas cotas em centímetros, mostrar até duas casas decimais: `8,6666 cm` e
  `8,6667 cm` são apresentados como `8,67 cm`. Esse arredondamento pertence
  somente às cotas; os campos de edição e a geometria persistida conservam sua
  precisão. As demais unidades mantêm a apresentação existente.
- No espaçamento padrão, mostrar uma cota no intervalo alcançado. No espaçamento
  igual, indicar os intervalos correspondentes com a mesma medida.
- Exibir somente as guias do resultado realmente alcançado, com identificação
  discreta da referência externa. Consolidar coincidências para evitar acúmulo
  de traços e rótulos.
- Guias e cotas ficam limitadas à superfície ativa e desaparecem na soltura,
  no cancelamento, na suspensão por `Ctrl` ou quando o alvo deixa de ser válido.
  Não interceptam o ponteiro nem alteram o cursor próprio do gesto.

### Consolidação e persistência

O snap produz somente a geometria final do gesto. Não cria uma ligação que
faça um Frame acompanhar alterações futuras de outro, nem uma obrigação de
manter aquele alinhamento ou espaçamento após outras edições.

Prévia, alvo, guias e estado de suspensão são transitórios. A soltura consolida
uma única ação de Undo/Redo; cancelamento e gesto sem alteração efetiva não
criam Histórico. Somente a geometria confirmada participa de Salvamento,
reabertura e Exportação.

Os snaps deste contrato pertencem ao movimento e ao redimensionamento diretos
de Frames. Inserção de Fotos, colagem e aplicação de Layouts conservam suas
próprias regras de geometria.

## Cenários de aceite

| Área | Verificação |
| --- | --- |
| Alvos de alinhamento | Bordas e centros de Frames, Páginas, Lâmina dupla, corte e segurança; Página única sem alvos no Lado inativo. |
| Dimensões | Igualdade de largura e altura em posições diferentes, oito alças, cantos independentes, `Shift`, `Alt` e combinação. |
| Espaçamento | Repetição de intervalo, padrão do Projeto e equilíbrio entre vizinhos; movimento e redimensionamento nos dois eixos. |
| Seleção múltipla | Somente a Caixa delimitadora gera encaixes; Frames internos não atraem o grupo; transformação coletiva e uma ação de Histórico. |
| Sensibilidade | Aquisição a `6 px`, retenção até `10 px`, saída deliberada, mudança de alvo e mesma sensação em escalas diferentes do Canvas. |
| Conflitos | Dois alvos próximos, referências coincidentes, empate entre tipos e proporção que não permite satisfazer largura e altura simultaneamente. |
| Restrições | Layout travado, superfície ativa, mínimo interativo, Frames antigos menores que o mínimo, não inversão e candidato inválido sem guia enganosa. |
| Modificadores e seleção | `Ctrl` pressionado ou solto durante o gesto, `Ctrl` + clique abaixo do limiar de arraste, mudança dinâmica de `Shift` e `Alt`. |
| Referências de Frame | Frame preenchido, placeholder, Opacidade zero, sobreposição e exclusão dos próprios Frames selecionados como alvos. |
| Guias | Traços e cotas legíveis, unidade correta, referência identificável, coincidências consolidadas e desaparecimento nos terminais do gesto. |
| Espaço entre Frames | Valor escolhido na criação e alterado em Design do Álbum; mesma medida nos snaps e no Gerador; preservação de Projetos existentes e composições já feitas. |
| Histórico e saída | Um gesto é uma ação; cancelamento e ausência de mudança não criam ação; Undo/Redo, Salvar/reabrir e Exportação conservam a geometria confirmada, sem guias ou vínculos de snap. |

## Validação da implementação

A entrega deve exercitar os cenários de aceite pela interface e pela fronteira
pública de edição do Projeto, incluindo os terminais de confirmação e cancelamento.
Os valores de sensibilidade e a legibilidade de guias e cotas devem ser calibrados
no Canvas real. Este contrato não substitui essa evidência visual nem declara
a implementação concluída.
