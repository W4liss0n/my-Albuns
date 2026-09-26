---
status: current
document: research
date: 2026-09-25
platform: windows-11-x64
---

# Perfil da edição no Canvas

O custo de cada ação criativa foi medido na Janela do Projeto para decidir se
vale enviar à interface apenas as Lâminas alteradas, em vez da projeção
inteira, que tem cerca de 290 KB para 30 Lâminas.

## Resultado

Álbum com 30 Lâminas, 5 Frames com foto por Lâmina e 172 fotos. Médias de dez
ações de cada tipo:

| Ação | Chamada ao Host | Script na interface | Tarefas da interface |
| --- | ---: | ---: | ---: |
| Arrastar um Frame (9 prévias e a confirmação) | 2,6 ms por prévia, 16 ms na confirmação | 44 ms | 85 ms |
| Undo | 14 ms | 26 ms | 42 ms |
| Redo | 14 ms | 26 ms | 41 ms |
| Trocar de Lâmina pela roda | — | 9 ms | 28 ms |

O perfil de CPU do JavaScript durante Undo e Redo mostrou 27 ms por ação:

| Parte | Tempo por ação |
| --- | ---: |
| React e bibliotecas da interface | 9,0 ms |
| Código do aplicativo, incluindo a cena do Canvas | 8,4 ms |
| Recebimento da resposta, leitura do JSON e coleta de memória | 5,4 ms |
| Renderização WebGL | 3,6 ms |

## Conclusão

Enviar apenas as Lâminas alteradas economizaria parte dos 5,4 ms de
recebimento por ação. Isso exigiria acompanhar, por janela, qual projeção cada
interface possui em todos os caminhos que devolvem uma projeção, com risco de
exibir um estado diferente do Projeto. A mudança não foi feita. Se álbuns muito
maiores tornarem a edição lenta, o primeiro alvo é a reconciliação da
interface, que pode preservar as Lâminas inalteradas entre projeções.

## Método

- Windows 11, Intel Core i5-13450HX, 16 processadores lógicos, 24 GB de RAM.
- Aplicativo compilado com `tauri build --debug --no-bundle`; a interface é o
  build de produção do Vite e o Host roda sem otimização completa.
- Porta de depuração do WebView do Projeto aberta por
  `MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT`. As ações foram enviadas pelo
  protocolo de depuração: duplo clique na Barra da Lâmina para editar, arrasto
  de 40 px em dez passos, `Ctrl+Z`, `Ctrl+Y` e roda do mouse.
- Os tempos da interface vêm de `Performance.getMetrics`; as chamadas ao Host,
  dos registros de recursos `ipc.localhost`; a divisão do script, de
  `Profiler` com amostragem a cada 100 µs.
