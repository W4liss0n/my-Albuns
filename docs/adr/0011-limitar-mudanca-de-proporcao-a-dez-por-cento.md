---
status: accepted
date: 2026-09-16
ticket: 5
---

# Limitar a mudança de proporção a 10%

O usuário aprovou em 16/09/2026 a tolerância de 10% depois de comparar formatos
quadrados, retangulares, limítrofes e uma troca de orientação. A Mudança
dimensional segura compara a proporção atual com a desejada de forma simétrica:
`D = max(r_atual / r_nova, r_nova / r_atual) − 1`. O limite inclusivo é `D ≤ 0,10`.
Esta é uma regra de produto para admitir formatos próximos, não uma garantia
de que o recorte de toda Foto permanecerá idêntico.

A escala física pode aumentar ou diminuir além de 10% quando a proporção se
mantém. Não há rotação automática do Álbum nem troca automática de largura e
altura para fazer uma conversão passar. Uma mudança entre retrato e paisagem
obedece ao mesmo cálculo, sem uma segunda restrição por rótulo de orientação.
Continuam valendo as validações físicas, de representação numérica e de
resolução já existentes.

## Consequências

- `20 × 30 → 25 × 35 cm` tem diferença de `7,142857…%` e é admitido.
- `30 × 40 → 30 × 44 cm` tem diferença de `10%` e é admitido no limite.
- `30 × 40 → 30 × 45 cm` tem diferença de `12,5%` e é recusado.
- `20 × 30 → 30 × 20 cm` tem diferença de `125%` e é recusado.
- As mesmas respostas valem na direção inversa e usando a largura da Lâmina
  aberta ou da Página, desde que ambas as proporções usem a mesma convenção.
- A comparação usa as medidas canônicas exatas. Arredondar a porcentagem para
  exibi-la não autoriza um valor acima do limite.
- Frames acompanham a mudança de cada eixo; Fotos mantêm escala uniforme,
  sem esticamento. O enquadramento é preservado quando possível, limitado pelo
  Preenchimento do Frame, conforme o [contrato da transformação](../design/0036-mudanca-dimensional-segura.md).
- A transformação é validada por inteiro antes da aplicação. Uma falha não
  deixa parte das Lâminas no formato novo; confirmar cria uma única ação de
  Undo/Redo e não salva automaticamente.

Permitir apenas proporções idênticas excluiria os formatos próximos aprovados.
Comparar somente a variação de largura ou de altura recusaria ampliações
proporcionais legítimas. Uma diferença calculada apenas sobre a razão original
produziria respostas distintas ao inverter origem e destino; por isso foi
adotada a comparação simétrica.
