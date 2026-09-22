---
status: accepted
document: design
date: 2026-09-22
implementation-readiness: ready-for-agent
---

# Controles compartilhados de formulários e menus

Esta decisão implementa as quatro centralizações da revisão de 18/09/2026,
aprovadas pelo usuário em 22/09/2026. O objetivo é retirar implementações
repetidas preservando as diferenças de cada fluxo.

## Navegação dos menus

`src/ui/menuNavigation.ts` concentra a identificação dos itens habilitados de um
menu, o foco inicial, a escolha do item selecionado, os extremos e a navegação
circular. Itens de um submenu não entram na navegação do menu pai.

O menu principal, os menus contextuais e o seletor de decorativos usam essa
base. Cada consumidor continua definindo suas teclas, abertura, fechamento e
restauração de foco. Setas laterais continuam trocando menus ou abrindo submenus
no menu principal e percorrendo opções no seletor de decorativos. Os menus
contextuais continuam usando apenas as setas verticais para percorrer itens.
`useDismissableSurface` mantém seu papel atual, sem incorporar essa navegação.

## Apresentação dos itens

`MenuItem` e `MenuSeparator`, em `src/ui`, possuem os rótulos, atalhos,
marcação de seleção, estados visuais e divisores. Os consumidores são os menus
principal, de quadro, de lâmina, de imagens, de pastas e de importação.

O menu Importar conserva sua apresentação compacta. Os catálogos continuam
donos dos nomes, atalhos e identidades de comandos. Disponibilidade, execução
e fechamento permanecem com os fluxos; o controle visual não despacha comandos.

## Campos com unidade

`ValidatedTextField` concentra o rótulo acessível, a entrada, a unidade opcional,
as ações laterais e a integração com `FieldValidationTooltip`. Ele substitui
as três composições numéricas locais da criação e das informações do álbum,
mantendo as versões regular e compacta.

O formulário conserva o rascunho, a conversão de medidas, o momento de mostrar
os erros e o agrupamento da validação. O botão Restaurar conserva o foco na
entrada. Erros continuam em tooltip, sem deslocar controles. Seletores de
opções e seus fluxos não são transformados em campos numéricos.

## Padrões de borda e espaço

`FrameDefaultRangeControl` possui a composição do slider, a apresentação do
valor e as faixas padrão de borda (0 a 5 mm, passo de 0,25 mm) e espaço (0 a
24 mm, passo de 1 mm). Personalização na criação e Design do álbum usam essa
mesma definição.

A criação conserva a faixa padrão. O álbum pode ampliar o máximo para
representar valores existentes, inclusive a espessura lembrada ao desativar
a borda. A regra de borda zero e preservação da cor continua em
`frameBorderEditor`. Paleta, amostra, rascunho e aplicação continuam nos fluxos.

## Limites e verificação

Não há alteração no contrato com Rust, nas regras do Core, nos valores
persistidos nem no histórico. Continuam válidos os designs 0003, 0037, 0040,
0041 e 0044, incluindo o padrão de tooltip e os controles compactos de design.

A verificação usa os testes dos consumidores: navegação com itens
desabilitados e submenus, seleção inicial do decorativo, comandos, edição de
medidas, unidades, restauração e aplicação dos padrões. A aceitação visual usa
os cenários declarados de menus, criação e painéis contextuais afetados.
