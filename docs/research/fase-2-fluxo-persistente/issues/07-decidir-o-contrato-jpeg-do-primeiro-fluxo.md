---
status: historical
document: technical-research
ticket: fase-2-fluxo-persistente-07
date: 2026-08-03
updated: 2026-08-05
---

# Decidir o contrato JPEG do primeiro fluxo

Type: grilling

Status: resolved

Blocked by: 02 — Delimitar o contrato JPEG verificável desta fase

## Question

Qual subconjunto do contrato recomendado pela pesquisa passa a ser normativo para a Exportação JPEG da Lâmina visível nesta fase, e quais capacidades permanecem explicitamente nos tickets posteriores?

A decisão deve fixar resultados observáveis e casos dourados suficientes para que Canvas e Exportação usem a mesma composição sem acoplar o Processador ao documento persistido ou ao Cache de visualização.

## Answer

Aprovado pelo responsável do produto em 2026-08-03. O design [Contrato JPEG do primeiro fluxo](../../../design/0014-contrato-jpeg-do-primeiro-fluxo.md) é a decisão normativa.

`Exportar Lâmina` é uma funcionalidade real e estreita: produz um único `.jpg` da Lâmina selecionada, usando exatamente a revisão visível inclusive não salva, com destino explícito e sem modificar o Projeto. O snapshot contém o `CompositionPlan` canônico calculado uma vez; o pipeline extrai somente a unidade selecionada e o Processador não recebe `.myalbuns`, Álbum, outras Lâminas ou Cache. Background e Overlay globais do schema v1 participam igualmente do Canvas e do JPEG; alfa do Background é composto sobre branco canônico, enquanto o Overlay preserva alfa até a composição.

A saída é JPEG/JFIF baseline, RGB8 opaco, qualidade fixa `100`, DPI do Projeto, perfil controlado `sRGB2014.icc`, sem metadados herdados e com o `4:2:2` atual aceito apenas como limitação transitória. Dimensões e divisões internas usam aritmética inteira e intervalos semiabertos. Fontes são sempre os originais JPEG/PNG estáticos: variantes RGB, YCbCr, tons de cinza, indexadas e de até 16 bits entram na matriz aprovada; CMYK/YCCK, APNG, TIFF e perfis fora da allowlist falham tipadamente antes da composição.

O fluxo usa `NativePathDto` reversível em toda a IPC, terminais `Completed | Failed`, verificação JPEG leve no Processador e confirmação independente de arquivo/tamanho/digest no host. Sobrescrita atravessa o plano como `CreateOnly` ou `ReplaceConfirmed`, e Publicação conserva o envelope limitado do ADR 0006. Dois guardrails provisórios de `134_217_728` pixels limitam separadamente a saída e a soma das fontes; excesso ou falha de reserva retorna `ResourceLimitExceeded` sem invalidar o Projeto.

Modal completo, múltiplas unidades, intervalos, Álbum inteiro, modo Por página, lote, PNG/PDF de saída, TIFF de entrada, animação, conversão ICC, cor profissional, `4:4:4`, tiles e orçamento definitivo de memória permanecem adiados. Os casos dourados do design fixam composição compartilhada, largura ímpar, Página única, orientação, transparência, matriz de fontes, estado visível, IPC nativa, falhas e ciclo de Publicação.
