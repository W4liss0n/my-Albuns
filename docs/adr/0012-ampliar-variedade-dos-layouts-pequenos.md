---
status: accepted
date: 2026-09-17
---

# Ampliar a variedade dos Layouts com poucos Frames

A revisão de 165 consultas encontrou 73 com menos de cinco sugestões. O
autor solicitou ampliar a variedade para buscar de cinco a dez opções e
permitir até vinte quando houver composições distintas e espaço físico.

A versão 2 mantém a busca determinística do ADR 0010 e amplia o teto para
vinte. A seleção principal preserva os critérios de qualidade e novidade.
Se retornar menos de cinco opções, uma etapa complementar busca atingir cinco,
mantendo a nota mínima e admitindo diversidade geométrica um pouco menor.
Essa etapa não elimina opções já selecionadas nem altera sua ordem.

Para um a seis Frames, a etapa complementar enumera composições em regiões
menores centralizadas e bandas desiguais ajustadas às proporções naturais.
Os intervalos físicos são recalculados; não são reduzidos junto com os Frames.
Um único Frame tem sua ocupação avaliada em relação ao maior Frame de
proporção natural que cabe em sua região, evitando penalizar uma Foto
vertical apenas porque a superfície é larga.

Quantidade e qualidade permanecem subordinadas às invariantes: orientação,
forma quadrada, margens, menor lado, intervalo, ausência de sobreposição,
centralização por Página e permissão de Travessia central. Cinco é uma meta,
não uma garantia absoluta. Restrições físicas ou falta de variedade podem
produzir menos opções; não se repete a mesma geometria para completar a lista.

As definições salvas continuam completas e independentes do algoritmo.
Último Layout, Favoritos, Personalizados, prévia, aplicação e reserva mantêm
seus contratos. Não há mudança de schema do Projeto nem controles técnicos
novos na interface.

Os parâmetros e as duas etapas constam no
[design 0026](../design/0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md).
A [revisão de base](../research/2026-09-17-diversidade-do-gerador-de-layouts.md)
registra as medições da versão anterior.
