---
status: accepted
date: 2026-09-30
updated: 2026-10-01
---

# Diversificar e harmonizar as sugestões do Gerador

A análise de 30/09/2026 rodou o Gerador aprovado na V9 em 1.150 consultas, com
Lâminas de 60 × 30, 50 × 25, 40 × 30 e 80 × 30 cm, Páginas únicas e 1 a 20 Frames.
Nove consultas comuns ficavam sem sugestão e caíam na reserva, que trocava V/H em
sete delas; quatro Frames da mesma orientação numa Página não tinham solução;
8,1% das sugestões por Página tinham blocos de alturas diferentes; e 2.473
posições das listas eram espelhamentos de outra sugestão. A primeira sugestão é
a que as automações aplicam sozinhas.

A versão 2 do algoritmo mantém as famílias e a busca determinística do
[ADR 0010](0010-gerar-layouts-por-composicoes-deterministicas.md) e muda:

- **Teto de vinte sugestões.** Com o teto de dez, 844 das 1.150 consultas eram
  cortadas no limite. Com vinte, a média sobe de 8,6 para 14,9 sugestões, sem
  mudar as dez primeiras nem a ordem. Com um a três Frames o teto não é o limite:
  faltam composições. O autor já tinha aprovado vinte em 17/09/2026, numa branch
  não integrada, e confirmou em 30/09/2026.
- **Ocupação e corte por escopo.** A ocupação é comparada com a maior área de
  Frames do mesmo escopo, e cada escopo tem sua janela de dez pontos abaixo da
  melhor nota, com o piso de 72. Um Frame único ou uma composição por Página deixa
  de ser julgado pela Lâmina inteira, e uma composição que atravessa o centro não
  expulsa as opções por Página, nem o contrário.
- **Horizonte comum.** Uma composição por Página perde até oito pontos quando os
  blocos das duas Páginas têm alturas diferentes.
- **Espelhamentos por último.** Uma sugestão igual a outra por espelhamento, por
  espelhamento de um bloco dentro da sua Página ou por troca das Páginas só
  entra depois das estruturas distintas.
- **Destaque e apoio.** Pilhas de um Frame junto a um Frame maior não contam como
  trilhas repetidas, e o destaque também é resolvido junto com seu apoio nas
  proporções de referência, centralizado, além das frações fixas.
- **Grade como último recurso.** Sem nenhuma outra composição, o Gerador devolve
  a melhor grade uniforme, que conserva orientação, margem e intervalo, em vez
  de deixar a automação para a reserva do
  [ADR 0008](0008-garantir-layout-compativel-por-arranjo-de-reserva.md).

A Travessia central continua decidida somente pela Permissão de Layouts do
Projeto. Uma regra que restringia a travessia a horizontais largas foi avaliada e
descartada pelo autor em 30/09/2026: com Páginas e Lâmina permitidas, um Frame
pode atravessar o centro em qualquer posição.

Nas mesmas consultas, nenhuma fica sem sugestão, as consultas com menos de cinco
sugestões caem de 205 para 164, as estruturas distintas por lista sobem de 6,2
para 13,9, os espelhamentos passam de 26% para 7% das posições das listas e as
Páginas desalinhadas caem de 8,1% para 2,3% das sugestões por Página. Em troca,
a primeira sugestão tem mais contraste de tamanho (um destaque com três apoios
pequenos) e o Painel prepara uma prévia para cada uma das até vinte sugestões.

O tempo fica no nível da versão 1. A seleção passou a atualizar a novidade de
cada candidato uma vez por escolha, em vez de recalculá-la contra todas as
escolhidas a cada rodada, e as chaves internas deixaram de ser montadas mais de
uma vez; as geometrias devolvidas são idênticas às da versão sem essas
otimizações nas 1.150 consultas. Medido no aplicativo de desenvolvimento, o
Painel de uma Lâmina com 20 Frames abre em cerca de 1,06 s (1,05 s na versão 1);
no build de release, a busca leva 169 ms (232 ms na versão 1). As vinte prévias
ficam prontas em cerca de 27 ms. Com o núcleo otimizado também no perfil de
desenvolvimento, as buscas repetidas reaproveitadas na sessão e o trabalho
independente feito em paralelo, o mesmo Painel abre em cerca de 0,15 s; as
medições estão em
[Desempenho do Painel de Layouts](../research/2026-09-30-desempenho-do-painel-de-layouts.md).

O autor aprovou a versão 2 em 01/10/2026, na revisão visual do PR que a
implementou. Detalhes e parâmetros estão no
[design 0026](../design/0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md).
Penalizar mais de três tamanhos e Frames muito pequenos foi avaliado e adiado:
reduzia o contraste, mas aumentava o corte das Fotos e diminuía a variedade.
A versão 3, do
[ADR 0015](0015-ampliar-as-sugestoes-com-pagina-inteira-e-proporcoes-reais.md),
retoma essas penalidades junto com composições que não cortam as Fotos.
