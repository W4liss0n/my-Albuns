---
status: current
document: research
date: 2026-09-08
ticket: 20
platform: windows
---

# Conferência da edição de Frames e próxima entrega

O histórico integrado até `e3dc461` entrega a base de edição de Frames e a
orientação de Fotos da PR #77. A issue
[#20 — Edição de Frames e Fotos](https://github.com/W4liss0n/my-Albuns/issues/20)
continua aberta, e seu checklist ainda não representa as entregas integradas.
Esta conferência distingue a base existente dos critérios que dependem de
capacidades posteriores; não declara a issue inteira concluída.

| Grupo | Evidência existente | Situação |
| --- | --- | --- |
| Seleção simples e múltipla, movimento e redimensionamento | PRs #66 e #67; testes públicos de geometria e integração do Canvas | Implementado no Modo de edição, com limites coletivos, modificadores e Histórico. |
| Pilha visual | PR #68; corpus e testes de ordenação de Frames | Quatro comandos de Organizar, ordem relativa e uma ação por comando. |
| Placeholder manual | PR #69; `manual_frame.rs` e cenários de criação manual | Inserção centralizada 3:2, adaptação à superfície e seleção do novo Frame. |
| Exclusão em Layout destravado | PR #70; testes de exclusão e seu corpus | Exclui a seleção no Modo de edição, preservando composição restante e mídias importadas. |
| Troca de conteúdo | PRs #71–#74; testes de troca e arraste no Modo normal | Fotos e ajustes acompanham a ocorrência; Frame e geometria permanecem. Inclui troca entre Lâminas e placeholder. |
| Copiar e colar | PR #75; `frame_clipboard.rs` e integração da fila | Reutiliza vínculos, adapta o conjunto à superfície e cria uma ação de Histórico. O escopo completo continua também relacionado à issue #34. |
| Troca de lados da Lâmina | PR #76; `sheet_side_swap.rs` e aceitação da Barra | Translaciona os Frames contidos em cada Página e preserva Travessias centrais, Numeração, conteúdo e ajustes. |
| Giro e Espelhamento | PR #77; `photo_orientation.rs` e corpus visual | Giro anti-horário em passos de 90°, restauração independente e Espelhamento horizontal, com Histórico e Projeto v4. |
| Ângulo fino | `photo_orientation.rs`, controle do Painel e design 0022 | Ajuste de −45° a 45°, prévia sem gravação, seleção múltipla, restauração por dois cliques e Projeto v5. |
| Inserção/substituição e Pan/Zoom | `photo_composition_v3.rs`, `usePhotoGestures.ts` e integração do Projeto | Alvo superior, prioridade dos placeholders, reinício dos ajustes da substituta e separação entre Foto e geometria. |

## Critérios que continuam vinculados a entregas posteriores

- **Layout travado — #26:** o Core ainda não persiste esse estado. Excluir somente
  as Fotos e conservar placeholders, bloquear criação/movimento/redimensionamento
  e impedir Troca de lados dependem dessa entrega. O indicador da Barra permanece
  preparado para o estado futuro; isso não comprova travamento implementado.
- **Aplicação completa de Layouts — #25:** a inserção normal já possui um arranjo
  determinístico, mas o catálogo, a escolha pela prioridade de Layouts e a
  reorganização completa após exclusões pertencem à entrega de aplicação.
- **Estilos e transformações — #22:** os recortes anteriores preservam os ajustes
  existentes. Borda/Opacidade individuais e Preto e branco continuam pendentes.
  Giro, Espelhamento e Ângulo fino já têm seus recortes implementados; preservar
  propriedades em Copiar ou Trocar, por si só, não entrega os controles restantes.

Esses critérios cruzam a sequência declarada de dependências
`#20 → #22 → #25 → #26`. O fechamento administrativo da #20 deve distinguir os
critérios da base de edição dos critérios de integração com essas capacidades,
mantendo os últimos nos tickets proprietários. Esta rodada conserva os tickets
abertos e documenta a separação; não elimina critérios nem altera dependências.

## Recortes atuais da issue #22

Giro de 90° anti-horário e Espelhamento horizontal usam a seleção já implementada,
comandos da mesma fila, Histórico e a composição compartilhada. O
[contrato da orientação e do Projeto v4](../design/0021-orientacao-de-fotos-e-projeto-v4.md)
registra o escopo, a seleção múltipla e a evolução necessária do arquivo persistente.
O [Ângulo fino e o Projeto v5](../design/0022-angulo-fino-da-foto-e-projeto-v5.md)
completam o recorte seguinte. Preto e branco, Borda e Opacidade continuam
pendentes na #22; aplicação de Layouts e travamento vêm depois, conforme a
sequência acima.
