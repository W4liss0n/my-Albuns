---
status: accepted
date: 2026-09-17
---

# Incluir espelhamentos e buscar dez Layouts

Após testar a versão 2, o autor considerou a variedade insuficiente e aprovou
buscar dez opções, admitir espelhamentos distintos e ampliar as composições
assimétricas, mantendo o teto de vinte sugestões.

A versão 3 preserva a seleção principal. Quando ela retorna menos de dez
opções, a etapa complementar tenta chegar a dez com nota mínima de 72 e
novidade mínima de 0,12. Para um a seis Frames, a enumeração acrescenta
escalas graduais centralizadas e repartições moderadamente desiguais.
O intervalo é recalculado em cada composição e conserva seu tamanho físico.

Depois, havendo lugar na lista, o Gerador considera as reflexões horizontal,
vertical e combinada das opções selecionadas. Acrescenta somente geometrias
distintas, com nota mínima de 72 e novidade de 0,05, até o teto de vinte.
A reflexão move os Frames conservando seus índices: não inverte as Fotos,
não altera seus ajustes e não troca conteúdos entre Frames. Uma composição
simétrica não ganha outra opção só por permutar Frames de mesma orientação.

Dez é uma meta de busca, subordinada à qualidade e às invariantes físicas.
Continuam obrigatórios orientação, forma quadrada, margens, menor lado,
intervalo, grupos completos, centralização por Página e permissão de
Travessia central. A lista pode ficar menor quando não há variedade válida.

As opções principais mantêm sua prioridade. Último Layout, Favoritos,
Personalizados, reserva, prévia, aplicação e histórico mantêm seus contratos.
As definições já salvas são independentes da versão do algoritmo; não há
alteração de schema ou novos controles técnicos no Painel.

Esta decisão atualiza a política do [ADR 0012](0012-ampliar-variedade-dos-layouts-pequenos.md).
Os parâmetros estão no [design 0026](../design/0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md)
e as contagens na [medição da versão 3](../research/2026-09-17-diversidade-do-gerador-de-layouts-v3.csv).
