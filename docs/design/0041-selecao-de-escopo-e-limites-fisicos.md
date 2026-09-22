---
status: accepted
document: design
date: 2026-09-17
platform: windows
implementation-readiness: ready-for-agent
---

# Seleção de escopo e limites físicos

O autor aprovou as duas oportunidades da revisão de UI em `120ebe50`.
Esta consolidação complementa o design 0040 e preserva o ADR 0005.

## Interação espacial

O [design 0049](0049-previa-compartilhada-de-escopo.md) unificou a superfície
completa em `VisualScopePreview`, com conteúdo geral ou lâmina real. O componente
possui seleção, disponibilidade dos lados, ponteiro, foco e apresentação dos
estados. As duas composições anteriores foram removidas.

### Destaque ao passar o mouse — refinamento de 22 de setembro

O ponteiro muda suavemente o tom da Página inteira, ou de ambas as Páginas
quando está na região central. O realce usa o tom neutro quente da interface
com opacidade de 12%, sem contorno, sombra ou recuo. A indicação alcança a
superfície completa e não depende apenas do contraste dos quadros de exemplo,
conservando a leitura na prévia pequena do painel contextual.

O realce aparece também quando Ambos os lados já está selecionado e quando o
ponteiro passa pela própria Página selecionada. Ele desaparece ao sair; não
troca o escopo nem altera o documento. A seleção azul continua visível na borda
externa e o foco de teclado mantém sua indicação própria.

`VisualScopePreview` possui esse realce compartilhado por Novo projeto,
Design do álbum e Design da lâmina. As áreas clicáveis permanecem iguais; o
realce de Ambos os lados cobre toda a lâmina, e não apenas o alvo central.

Nos dois painéis, os alvos usam 40% para cada lado e 20% para Ambos, sem
sobreposição. O foco e o hover cobrem a página inteira. Na criação permanecem
duas metades, com Ambos fora da lâmina. Página única só oferece o lado ativo.
Essa unificação substitui as três geometrias preservadas em 17 de setembro.

Na criação, Ambos continua sendo escolhido fora da Lâmina e o foco do painel
externo continua controlado pela etapa. Os realces de foco e seleção conservam
os estilos aceitos nos designs 0001 e 0003. Não há mudança de paleta, tipografia,
densidade, espaçamento ou Hierarquia; a Lâmina permanece o centro da interação.
Selecionar escopo não envia comando nem altera o Histórico.

## Intervalos físicos do Core

A validação de configuração produz os intervalos físicos de largura aberta e
altura admissíveis para o DPI informado. O mesmo cálculo aceita ou rejeita as
dimensões. Largura continua exigindo paridade e raster válido tanto da Lâmina
quanto de sua metade. DPI inválido produz limites ausentes.

Novo Projeto e Informações do Álbum recebem esses fatos na resposta existente,
inclusive quando outra regra impede a alteração. Não há chamada IPC adicional,
persistência de limites, nova versão de arquivo ou regra de documento em CSS/TS.
O host encaminha diretamente a validação de configuração pertencente ao Core.

A UI formata a Unidade pendente e divide a largura por dois quando apresenta
a Lâmina fechada. Mensagem, arredondamento da exibição e tooltip permanecem na
UI. A fórmula inversa raster e suas constantes deixam o frontend. O adaptador
de criação recusa respostas sem limites utilizáveis quando há erro raster;
não inventa valores substitutos.

## Verificação

- Intervalos exatos nos 1.200 DPIs válidos: extremos aceitos, imediatamente fora
  rejeitado e paridade da largura preservada; DPI inválido sem intervalo.
- Criação real e validação pública do Álbum nos extremos com DPI 1, 300 e 1.200;
  consulta sem mutação da projeção e raster final dentro do contrato.
- Contratos gerados, encaminhamento nativo, apresentação em mm/cm/polegadas e
  largura aberta/fechada; fatos fornecidos pelo Core sem reconstrução na UI.
- Ponteiro, teclado, seleção e Página única; fluxos existentes de criação,
  Design do Álbum e Lâmina; capturas dos cenários afetados do manifesto.

## Contrato externo consultado

React instalado: 19.2.8. A skill `find-docs` consultou o Context7 para a linha
19.2 (fonte versionada disponível: 19.2.7), cobrindo estado controlado,
composição por propriedades e eventos de foco. Mantêm-se os contratos de
eventos e estado existentes, sem dependência ou API nova.
