---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-29
updated: 2026-07-29
---

# Baseline preliminar das topologias

Coletado em UTC: `2026-07-29T16:15:11.5288986Z`.
[JSON bruto](0001-topology-spike-baseline.json).

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos na árvore | 11 | 9 |
| Working set agregado | 591,3 MiB | 543,9 MiB |
| Memória privada agregada | 302,7 MiB | 289,1 MiB |
| Memória gráfica compartilhada | 86,4 MiB | 88,8 MiB |
| Primeiro host de A identificado | 660 ms | não se aplica |
| Duas Janelas identificadas | 1047 ms | 667 ms |
| Janelas depois da queda forçada | 1 (outra Janela preservada) | 0 |

## Build medida

- Commit do código: `384463c335e35a546c33e035cfd3863f1b112f1b`
- Build concluída em UTC: `2026-07-29T16:14:25.2702985Z`
- Perfil: `debug`
- Árvore de trabalho tinha mudanças alheias: sim
- Entradas da build tinham mudanças: não
- Arquivos de entrada: 128
- Digest das entradas: `2ba7370d910ba27b8c74c463125c952fbca255f9c91b87b79036459894c204f8`
- Hash do executável: `49d5a862c3519c7b8b0b0b5fe167dd08b01da4d90ff4558f8c111424e760d043`
- Checkout atual corresponde ao manifesto: sim

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
