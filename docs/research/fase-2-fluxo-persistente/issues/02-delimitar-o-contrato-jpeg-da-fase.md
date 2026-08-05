---
status: historical
document: technical-research
ticket: fase-2-fluxo-persistente-02
date: 2026-08-03
updated: 2026-08-05
---

# Delimitar o contrato JPEG verificável desta fase

Type: research

Status: resolved

Blocked by: None

## Question

Qual é o menor contrato JPEG tecnicamente defensável para exportar a Lâmina visível neste primeiro fluxo, considerando dimensões físicas e DPI, qualidade, alfa e fundo, orientação, espaço de cor, metadados e casos dourados, sem declarar concluído o escopo mais amplo do `Renderizador final`?

A pesquisa deve usar fontes primárias, recomendar uma fronteira verificável para esta fase e listar explicitamente o que permanece adiado.

## Answer

A pesquisa está em [Contrato JPEG do primeiro fluxo real](../../0031-contrato-jpeg-do-primeiro-fluxo.md), produzida isoladamente no commit `cfe9c55ae6d4bc618b847db6179acc3e4b945c9d` da branch `codex/research-jpeg-contract-phase2` e incorporada integralmente a este worktree.

A fronteira recomendada é um JPEG/JFIF baseline opaco, RGB de 8 bits, sRGB declarado, qualidade fixa 100, densidade do Projeto no APP0 e dimensões calculadas por aritmética inteira determinística. A orientação é aplicada uma vez na entrada; alfa é composto sobre o Background antes de RGB; EXIF, XMP, GPS e thumbnails das fontes não são copiados.

O gate deve reabrir o arquivo e verificar estrutura, dimensões, DPI, cor e casos dourados semanticamente, sem transformar o hash dos bytes comprimidos em contrato permanente. O encoder atual pode ser usado sem dependência nova, mas seu `4:2:2` permanece uma limitação explícita desta fase.

Conversão ampla de perfis, `4:4:4`, slider de qualidade, PNG, PDF, paginação, lote e o Renderizador final completo permanecem adiados. Esta resposta é evidência para `Decidir o contrato JPEG do primeiro fluxo`; ela não torna a recomendação normativa por si só.
