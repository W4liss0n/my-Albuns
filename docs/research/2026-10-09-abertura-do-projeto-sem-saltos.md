---
status: current
document: research
date: 2026-10-09
platform: windows-11-x64
---

# Abertura do Projeto sem saltos

Relato de 8/10/2026: ao abrir um Projeto, as Lâminas do Canvas davam pequenos
saltos enquanto a janela se ajustava, primeiro para o lado e, depois da
primeira correção, na altura. A base de código é o commit `859af5b9`.

## Como foi medido

- Compilação de desenvolvimento (`--debug`), com dados isolados em
  `MYALBUNS_PROCESS_GATE_DATA_ROOT` e a posição de janela do usuário
  (maximizada) copiada para eles.
- Projeto de teste local com 40 fotos em dez Lâminas.
- Pelo DevTools do WebView2, desde o primeiro instante da página: cada mudança
  de `innerHeight`, do tamanho do Canvas, da escala e da posição da cena e da
  Lâmina centralizada.
- Ao mesmo tempo, fora do aplicativo, uma leitura contínua da janela nativa:
  visível, maximizada (`IsZoomed`), encoberta pelo DWM e retângulo do cliente.
- Tela de 1920 × 1080 com a barra de tarefas do Windows 11: a área de trabalho
  tem 1920 × 1032.

## O que acontecia

| Momento | Antes | Depois |
| --- | --- | --- |
| Primeiro desenho, janela oculta | cliente de 1904 × 1023, escala 2,297 | cliente de 1920 × 1032, escala 2,327 |
| Janela aparece | a Lâmina anda 16 px | nada muda |
| Primeiros 200 ms | quatro alternâncias 1039 ↔ 1032 de altura, 7 px de lado a cada uma | nada muda |

Quatro causas se somavam:

1. **Deslocamento em pixels.** O deslocamento horizontal do Canvas é guardado
   em pixels. Quando a escala mudava, as Lâminas escalavam a partir da borda
   esquerda e andavam para o lado. Agora o conteúdo que está no centro do Canvas
   fica no centro quando o tamanho muda; uma navegação feita no meio tempo
   prevalece.
2. **Janela oculta do tamanho errado.** A janela que reabre maximizada era
   posta oculta na área de trabalho, mas a moldura invisível da janela sem
   bordas ficava dentro dessa área, e a página se diagramava 16 × 17 px menor.
   Agora o cliente da janela oculta coincide com a área de trabalho em tamanho e
   posição.
3. **`SetWindowPlacement` passa pelo tamanho restaurado.** Uma janela ainda não
   maximizada vai primeiro para os limites restaurados que recebe e só depois é
   maximizada. Mesmo numa janela já maximizada, gravar outros limites passa por
   eles internamente: o `tao` informa esse tamanho e o redimensionamento
   automático do Tauri leva o WebView junto por um instante (1039 px).
4. **`tao` restaura ao mostrar.** Uma janela criada sem foco é mostrada com
   `SW_SHOWNOACTIVATE`, que, como `SW_SHOWNORMAL`, restaura uma janela
   maximizada; o `tao` a maximiza de novo em seguida. Chamado de outra thread,
   o `show` chegava depois da maximização feita pelo Windows.

## Como a janela é apresentada agora

Toda a apresentação roda na thread principal. Para uma janela que reabre
maximizada:

1. o redimensionamento automático dos WebViews é suspenso;
2. a janela é encoberta pelo DWM, como já acontecia com os Projetos abertos
   atrás de outro;
3. o `tao` a mostra, ainda não maximizada;
4. `SetWindowPlacement` grava os limites restaurados lembrados e a maximiza;
5. a janela é descoberta, uma única vez, já final;
6. numa tarefa seguinte da thread principal, depois dos tamanhos intermediários
   que o `tao` informou, os WebViews voltam ao tamanho da janela e ao
   redimensionamento automático.

Em cinco aberturas seguidas, todas as etapas intermediárias aconteceram com a
janela encoberta; ela foi descoberta uma vez, maximizada e com cliente de
1920 × 1032, e a página manteve `innerHeight` de 1032 e a mesma escala do
primeiro desenho até o fim. Restaurar a janela depois volta aos limites
lembrados. O autor aprovou no aplicativo de depuração.

As medições e os roteiros ficaram fora do Git, em uma pasta local de
investigação.
