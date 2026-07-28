---
status: accepted
date: 2026-07-28
---

# Garantir sempre um Layout compatível por arranjo de reserva

Várias ações da SPEC reorganizam Frames sem escolha do usuário e são especificadas como "aplicar o primeiro Layout compatível": excluir ou acrescentar um Frame fora do Modo de edição, converter uma extremidade entre Lâmina dupla e Página única, e soltar uma Foto em área vazia no modo normal. Nenhuma delas possui comportamento alternativo definido para o caso de não existir candidato.

Ao mesmo tempo, o algoritmo do Gerador de Layouts está deliberadamente adiado. Sem uma garantia declarada, cada automação precisaria definir seu próprio caminho de falha antes que o Gerador existisse.

## Decisão

A disponibilidade de ao menos um Layout compatível é uma garantia absoluta do módulo `LayoutRules`, para qualquer quantidade suportada de Frames, formato de superfície e escopo aplicável.

Quando nenhum candidato do Catálogo global, dos Favoritos do Projeto ou do Gerador servir, `LayoutRules` produz um **arranjo de reserva** determinístico, derivado apenas da quantidade de Frames e da superfície ativa. O arranjo de reserva não é um Layout do catálogo: ele não recebe preview própria, não pode ser favoritado e não aparece como candidato no Painel de Layouts.

A garantia pertence a `LayoutRules`, não ao Gerador. O Gerador é um fornecedor de candidatos entre outros, e sua ausência, falha ou adiamento nunca pode propagar para uma automação.

## Alternativas consideradas

Permitir que a busca falhe e avisar o usuário foi rejeitado. Ele obrigaria cada automação a definir o que acontece na falha — o Frame excluído volta? a Lâmina fica sem organização? a ação é recusada? — multiplicando estados de erro em fluxos que o usuário não iniciou explicitamente e que hoje são silenciosos por design.

## Consequências

- As automações da SPEC permanecem totais: nenhuma precisa de caminho de falha por ausência de Layout.
- O ticket 06 decide diversidade, ranqueamento e qualidade dos candidatos, mas não pode decidir se a garantia existe — ela é anterior a ele.
- Os tickets que dependem da garantia — 17, 20, 21 e 26 — podem ser construídos antes do Gerador, usando o conjunto versionado de Layouts de teste mais o arranjo de reserva.
- O arranjo de reserva precisa de teste próprio de contrato: para toda quantidade suportada de Frames e ambos os escopos, `LayoutRules` devolve pelo menos um candidato.
- Um arranjo de reserva feio é um resultado aceitável; um arranjo de reserva ausente não é. A qualidade estética é responsabilidade do Gerador, a totalidade é responsabilidade de `LayoutRules`.
- A garantia não altera a regra de compatibilidade de travamento: um Layout com menos posições que Frames continua incompatível, e o arranjo de reserva nunca é usado para justificar perda de Frames.
