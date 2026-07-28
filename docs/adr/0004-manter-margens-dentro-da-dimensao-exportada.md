---
status: accepted
date: 2026-07-27
---

# Manter as margens de acabamento dentro da dimensão exportada

A Dimensão da Lâmina representa toda a superfície de uma Lâmina dupla e de sua saída por Lâmina. Sangria e Área de segurança são recuos internos uniformes, configurados na Unidade do Projeto, e nunca aumentam a largura ou a altura da unidade exportada. Saídas por Página e saídas de Lâminas de página única usam a Dimensão da Página. A Exportação inclui a superfície completa da unidade correspondente; as linhas de corte e segurança existem apenas como guias de edição e não são renderizadas.

## Consequências

- Uma Lâmina dupla configurada como `60 × 24 cm` é exportada por Lâmina com `60 × 24 cm`; cada saída por Página e uma Lâmina de página única medem `30 × 24 cm`. Sangria e segurança não ampliam nenhuma dessas unidades.
- O valor da Sangria é medido da borda externa para dentro, e o valor de segurança é acumulado a partir da linha de corte.
- A visualização normal mascara a Sangria, enquanto o Modo de edição da Lâmina mostra a superfície completa e suas guias.
- Frames e imagens podem ocupar a Sangria e atravessar a linha de segurança; somente a borda externa limita a composição.
- A divisão central de uma Lâmina dupla não recebe margens adicionais.
- Em uma Lâmina de página única, a borda voltada ao lado inativo também não recebe margens.
- Alterar os valores não transforma a geometria da composição, mas altera máscara e guias, participa de Undo/Redo e exige salvar o Projeto.
