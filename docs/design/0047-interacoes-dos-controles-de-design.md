---
status: accepted
document: design
date: 2026-09-22
implementation-readiness: ready-for-agent
---

# Interações dos controles de design

A revisão de 22/09/2026 identificou cinco inconsistências entre a criação do
projeto e os painéis de álbum, lâmina e quadro. O usuário aprovou corrigi-las
em conjunto e escolheu abrir a paleta completa do programa no primeiro clique.
Esta decisão substitui a etapa intermediária de escolha de cor dos designs
0024 e 0044, preservando a apresentação compacta e as operações existentes.

## Cor

`ColorPropertyControl` é o seletor comum: um clique na amostra abre a área de
saturação e brilho, a faixa de tom e o código hexadecimal. Não há outro seletor
para abrir dentro dessa superfície. A paleta usa os componentes acessíveis
do React Aria já instalado, com linguagem portuguesa, navegação por teclado,
posicionamento que se ajusta ao espaço disponível e retorno de foco ao fechar.

Aplicar confirma uma escolha; cancelar, Escape, sair da paleta ou trocar o
escopo descarta a escolha pendente. Confirmar a cor inicial em uma seleção
mista é uma ação explícita válida. Prévias do quadro continuam temporárias e
produzem somente uma entrada no histórico quando confirmadas. Álbum e criação
continuam com seus rascunhos e suas ações de Aplicar/Criar projeto.

A criação mantém as amostras rápidas e também permite escolher livremente a
cor da borda. Escolher uma cor com espessura zero conserva essa preferência
para quando a borda voltar a ter espessura, sem acrescentar uma borda sozinho.

## Medidas

Criação e álbum usam campo numérico e slider para borda e espaço entre quadros,
na mesma apresentação do quadro. `NumericRangeField` concentra a apresentação
e a posição do tooltip; os controles mantêm separados o ciclo de rascunho do
formulário e os gestos de prévia/histórico do editor.

Valores digitados aceitam vírgula ou ponto e conservam a precisão em micrômetros.
Os limites de criação continuam de 0 a 5 mm para borda e de 0 a 24 mm para
espaçamento, apresentados na unidade escolhida. O álbum conserva valores
existentes acima desses limites. O slider mantém seus passos usuais; esses
passos não restringem a precisão do campo digitado. Enter/saída normalizam o
campo válido; Escape restaura o valor anterior à digitação. Texto inválido
nunca entra no rascunho do projeto e recebe o tooltip do design 0040.

## Valores distintos e identificação

Uma seleção com duas cores diferentes mostra a amostra dividida entre elas.
Imagem, ausência e seleção mista permanecem distinguíveis; uma mistura não
marca uma cor branca nem a opção “Sem sobreposição”. A descrição fica no
tooltip e no nome/descrição acessível, sem inserir avisos no fluxo do painel.
`summarizeVisualSelection` concentra essa leitura de apresentação. Valores
iguais com origens diferentes têm uma amostra uniforme, mantendo a descrição
de herança e a restauração no painel da lâmina.

O painel do quadro deixa de exibir seu identificador interno. A identificação
legível continua pelo nome da foto ou “Quadro vazio”. IDs, contratos do Rust,
operações e persistência não mudam.

## Verificação e fontes

Testes dos fluxos públicos cobrem rascunhos, cancelamento, escopo, seleção mista,
precisão e unidades, histórico e criação com cor livre. A aceitação visual
usa os cenários declarados do repositório, incluindo paletas em janela menor,
cores distintas, campos inválidos e os painéis de quadro/foto afetados.

As APIs foram conferidas para React Aria Components 1.19.0 nas fontes oficiais:
[ColorArea](https://react-aria.adobe.com/ColorArea),
[ColorSlider](https://react-aria.adobe.com/ColorSlider),
[Popover](https://react-aria.adobe.com/Popover) e
[internacionalização](https://react-aria.adobe.com/quality).
