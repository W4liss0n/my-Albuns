---
status: accepted
document: design
date: 2026-09-09
ticket: 28
---

# Integração dos Layouts e schema v8

Este documento registra a primeira integração do
[contrato do Gerador](0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md).
O Gerador, `LayoutRules` e o Painel de Layouts passam a participar da sessão
real do editor. Favoritos, criação de Personalizados e trava de Layout
continuam nas suas entregas próprias.

## Fluxo no editor

O botão central da Barra da Lâmina abre um único painel horizontal acima do
Canvas. O painel conserva o alvo enquanto o usuário navega, recolhe durante
a edição da Lâmina e consulta novamente esse alvo ao voltar ao modo Normal.
Trocar o alvo, fechar o painel ou mudar a revisão descarta a prévia local.

`query_layouts` captura a Lâmina, revisão, ordem dos Frames e parâmetros do
Projeto. A sessão guarda os `LayoutPatch` resolvidos pelo núcleo e retorna um
identificador de consulta. `preview_layout` compõe cada opção pelo
`CompositionCore`, com as Fotos, placeholders, estilos e ajustes existentes.
Essas operações não alteram o Projeto, o estado de Salvamento ou o Histórico.

O hover exibe essa composição. Confirmar envia somente o identificador da
consulta e o índice exibido. `ApplyLayout` confere a revisão e os Frames e
aplica exatamente o patch preparado, como uma única ação de Histórico.
Uma consulta nova substitui a anterior. Uma confirmação obsoleta é recusada;
a interface não troca silenciosamente sua geometria por outra opção.

As consultas aguardam a fila compartilhada de mutações. A confirmação entra
na mesma fila de Salvar, Undo e Redo, conservando o identificador que o usuário
viu. Respostas atrasadas não substituem um alvo ou uma revisão mais recente.

## Automações

A inserção normal de um novo Frame, a exclusão normal e a conversão de uma
extremidade consultam `LayoutRules`. A conversão funciona tanto pelo comando
da Lâmina quanto por Informações do Álbum. Converter as duas extremidades
nesse formulário continua sendo uma só ação de Histórico.

Um Frame começa com a proporção manual de 3:2; a consulta não usa um quadrado
temporário nem a proporção da Foto. Preencher um placeholder ou substituir
sua Foto conserva a geometria. Na edição da Lâmina, adicionar ou excluir
Frames conserva a disposição manual dos demais.

O Último Layout compatível tem prioridade. Sem ele, vale a primeira sugestão
do Gerador; sem sugestão, a reserva do ADR 0008 reorganiza os Frames.
A reserva não aparece no painel e não substitui o Último Layout registrado.
A aplicação preserva ordem, IDs, conteúdo, Borda, Opacidade e ajustes das Fotos.

Os ajustes do painel pertencem ao Projeto: permissão por Página ou por Página
e Lâmina, margem, intervalo e menor lado. As medidas usam a Unidade de
apresentação, mas chegam ao núcleo em micrômetros inteiros. Alterá-las atualiza
as opções, sem reorganizar a composição atual. Valores padrão: ambos os
escopos, 15 mm de margem, 5 mm de intervalo e 20 mm de menor lado.

## Formato persistido

O [design 0028](0028-layout-travado-e-schema-v9.md) acrescenta o travamento
persistido na v9; a v8 abaixo permanece como contrato histórico de leitura.

O escritor passa a emitir `schemaVersion: 8`. O envelope e os campos de Foto,
estilo e caminhos Windows continuam nos contratos anteriores. A v8 acrescenta:

| Local | Campo obrigatório | Conteúdo |
| --- | --- | --- |
| `project` | `layoutSettings` | `permission`, `marginUm`, `gapUm`, `minimumSideUm` |
| Cada Lâmina | `lastLayout` | `null` ou uma definição com sua origem |

Uma definição guarda `surface` (`type`, `widthUm`, `heightUm`), `scope` e
`positions`, a sequência ordenada de retângulos físicos. A origem é
`automatic` ou `custom`. Não guarda IDs de Frames, Fotos, DPI, nota, pesos,
identificador de consulta ou estado de hover. A sequência pertence à definição:
trocar posições entre índices representa outro Layout.

O registro do Último Layout conserva a definição aplicada, mesmo após edição
manual dos Frames. A compatibilidade é reavaliada para quantidade, superfície
e permissão atuais. Reaplicar recupera a geometria original, com escala
proporcional quando aplicável; não reexecuta a versão que a gerou.

Os DTOs da v8 são fechados. Campos desconhecidos, definições vazias ou fora da
superfície e parâmetros inválidos são rejeitados. `lastLayout` pode ser `null`,
mas sua ausência é erro. Abrir um documento inválido não reescreve seus bytes.

O leitor continua aceitando v1 a v7 pela cadeia existente, inicializa os
parâmetros padrão e deixa `lastLayout` nulo. A migração ocorre em memória;
somente um Salvamento explícito escreve v8. A correção autorizada de identidade
de uma cópia antiga conserva seu schema de origem.

## Verificação

Os testes públicos cobrem as invariantes do Gerador de 1 a 30 Frames, escala,
medidas ímpares, quadrados, escopos, prioridade e reserva. A sessão verifica
prévia sem mutação, confirmação exata, revisão obsoleta, Undo/Redo, conversões,
exclusão, Salvamento, reabertura e rejeição de payloads incompletos.

O corpus `tests/fixtures/layout-panel-cases.json` é produzido e conferido pelo
`ProjectCore`. O adaptador visual apenas reproduz essas composições. Os
cenários `workspace-layout-*` do manifesto de aceitação exercitam o painel,
hover, Último Layout, Página única, ausência de sugestões, ajustes e alternância
de modo, incluindo apresentação a 125% e 150%.
