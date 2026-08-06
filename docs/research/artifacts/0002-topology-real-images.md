---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-29
updated: 2026-07-29
---

# Carga real preliminar das topologias

Coletado em UTC: `2026-07-29T19:15:16.5860323Z`.
[JSON bruto](0002-topology-real-images.json).

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos na árvore | 9 | 8 |
| Working set agregado | 1.121,7 MiB | 1.067,8 MiB |
| Memória privada agregada | 720,0 MiB | 709,0 MiB |
| Memória gráfica compartilhada | 443,1 MiB | 299,2 MiB |
| Primeiro host de A identificado | 1603 ms | não se aplica |
| Duas Janelas identificadas | 1818 ms | 399 ms |
| Duas Janelas com Cache pronto | 45324 ms | 22846 ms |
| Duração de parede do Cache frio | 43408 ms | 22201 ms |
| Fotos processadas pelo Cache | 172 | 172 |
| Vazão agregada dos originais | 32,3 MiB/s | 63,1 MiB/s |
| Representações reduzidas | 49,3 MiB | 49,3 MiB |
| Janelas depois da queda forçada | 1 (outra Janela preservada) | 0 |

## Corpus real

- Álbuns: 2
- Fotos JPEG: 172
- Volume dos originais: 1.401,0 MiB
- Digest do corpus: `4da593ff7d12f861a8b3aba74249726e70131468a68af7a7d06200f5340871cc`
- Integridade antes/depois: confirmada por SHA-256
- Política da representação: uma prévia JPEG por Foto, com aresta máxima de 1600 px

## Build medida

- Commit do código: `56f95ca5421dfeca667517944d7b9ba186486cd0`
- Build concluída em UTC: `2026-07-29T19:13:54.8393105Z`
- Perfil: `release`
- Árvore de trabalho tinha mudanças alheias: sim
- Entradas da build tinham mudanças: não
- Arquivos de entrada: 132
- Digest das entradas: `e3b1c016989bdae7aa4405989916025166b85718089471632419bba6a3926ca4`
- Hash do host: `a29a79086f407e3c1fb5f34b0623e4b8d080727ecadec0f8cda1635c386cebd5`
- Hash do Processador de Imagens: `7fdeb9903ac18464ead5d8e90a415a0e4e8fe8ac57adc7a1773fc776afa52999`
- Checkout atual corresponde ao manifesto: sim

## Ambiente registrado

- Sistema: Microsoft Windows 11 Pro `10.0.26200`
- Processador: 13th Gen Intel(R) Core(TM) i5-13450HX
- Memória física: 24.260,7 MiB

## Campos ainda não medidos

- latência de Pan/Zoom
- duração da Exportação
- recuperação persistida
- complexidade operacional da IPC

## Observações

- Carga preliminar com dois Álbuns reais; isto não encerra o spike.
- O Cache foi reconstruído a frio com uma representação JPEG de até 1600 px por Foto.
- Cada uso valida o SHA-256 da Foto antes de gerar ou reutilizar a representação; o corpus completo foi recalculado depois das duas alternativas.
- A memória inclui o host e todos os processos descendentes observados.
- A queda só é forçada depois de validar o caminho do executável do PID alvo.
- Os hosts independentes são iniciados em sequência depois que uma tentativa simultânea deixou intermitentemente um host sem Janela visível.
