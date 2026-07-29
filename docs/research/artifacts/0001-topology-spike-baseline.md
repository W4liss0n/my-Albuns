---
status: current
document: generated-research-artifact
ticket: 01-plataforma-e-arquitetura
date: 2026-07-29
updated: 2026-07-29
---

# Baseline preliminar das topologias

Coletado em UTC: `2026-07-29T16:06:11.3511340Z`.
[JSON bruto](0001-topology-spike-baseline.json).

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos na árvore | 11 | 9 |
| Working set agregado | 592,7 MiB | 544,8 MiB |
| Memória privada agregada | 301,2 MiB | 294,0 MiB |
| Memória gráfica compartilhada | 89,3 MiB | 88,8 MiB |
| Primeiro host de A identificado | 2766 ms | não se aplica |
| Duas Janelas identificadas | 3202 ms | 634 ms |
| Janelas depois da queda forçada | 1 (outra Janela preservada) | 0 |

## Build medida

- Commit do código: `0bf3e65a81950c055a842b53ddc3b0724c7aac53`
- Perfil: `debug`
- Árvore de trabalho tinha mudanças alheias: sim
- Entradas da build tinham mudanças: não

## Ambiente registrado

- Sistema: Microsoft Windows 11 Pro `10.0.26200`
- Processador: 13th Gen Intel(R) Core(TM) i5-13450HX
- Memória física: 24.260,7 MiB

## Campos ainda não medidos

- latência de Pan/Zoom
- vazão do Cache
- duração da Exportação
- recuperação persistida
- complexidade operacional da IPC

## Observações

- Baseline preliminar do esqueleto de topologia; isto não encerra o spike.
- A memória inclui o host e todos os processos descendentes observados.
- A queda só é forçada depois de validar o caminho do executável do PID alvo.
- Os hosts independentes são iniciados em sequência depois que uma tentativa simultânea deixou intermitentemente um host sem Janela visível.
