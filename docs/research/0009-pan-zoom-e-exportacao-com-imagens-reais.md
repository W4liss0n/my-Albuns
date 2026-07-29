---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-29
updated: 2026-07-29
---

# Pan, Zoom e Exportação com imagens reais

## Resumo

Esta rodada completa a medição comparável de dois Projetos nas alternativas de
host independente e host multiwindow. O mesmo corpus da rodada de Cache foi
usado: 172 Fotos JPEG de dois Álbuns, totalizando 1.401,0 MiB.

Pan e Zoom foram exercitados no Canvas PixiJS sobre representações reais do
Cache. A Exportação abriu os originais vinculados, validou tamanho e SHA-256 e
renderizou a primeira Lâmina do Projeto principal a 300 DPI. As duas topologias
produziram o mesmo PNG, byte por byte.

Os dados brutos estão em
[0003-topology-interaction-export.json](artifacts/0003-topology-interaction-export.json).
O [resumo gerado](artifacts/0003-topology-interaction-export.md) deriva do
mesmo objeto e concentra os valores da coleta.

## Método

O comando `npm run spike:topology` executou uma build `release`, registrou o
commit, o digest das entradas e os hashes do host e do Processador de Imagens.
O manifesto confirma que os insumos da build estavam limpos e que o checkout
medido corresponde ao executável.

Para cada alternativa, o instrumento:

1. limpou somente os namespaces descartáveis dos dois Projetos;
2. abriu os dois Projetos e reconstruiu uma representação JPEG de até 1600 px
   para cada Foto;
3. manteve fechado um gate comum às duas Janelas até ambos os caches
   terminarem;
4. esperou todas as texturas do viewport concluírem carga ou falha;
5. aqueceu cada Canvas por 24 frames;
6. mediu separadamente 120 frames de Pan e 120 frames de Zoom por Projeto,
   usando uma Foto com textura real;
7. exportou a primeira Lâmina do Projeto principal a 300 DPI, por snapshot
   validado e a partir de dois originais;
8. amostrou a árvore de processos e os contadores gráficos;
9. forçou a queda do host somente depois de validar o caminho do executável;
10. recalculou o digest do corpus antes de publicar o relatório.

Pan e Zoom usam a mesma transformação de prévia das interações reais, mas o
probe não consolida comandos nem cria entradas no Histórico. As duas operações
foram separadas para que uma não contaminasse as amostras da outra.

## Resultado

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Duas Janelas identificadas | 1.774 ms | 327 ms |
| Duas Janelas com Cache pronto | 25.409 ms | 24.219 ms |
| Cache frio | 23.714 ms | 23.572 ms |
| Vazão dos originais | 59,1 MiB/s | 59,4 MiB/s |
| Pan, média ponderada | 7,664 ms | 7,340 ms |
| Pan, pior p95 | 7,9 ms | 8,0 ms |
| Pan, frames acima de 16,67 ms | 0 de 240 | 0 de 240 |
| Zoom, média ponderada | 7,657 ms | 7,580 ms |
| Zoom, pior p95 | 8,0 ms | 8,0 ms |
| Zoom, frames acima de 16,67 ms | 0 de 240 | 0 de 240 |
| Exportação a 300 DPI | 1.113 ms | 1.130 ms |
| Working set agregado | 1.505,7 MiB | 1.437,8 MiB |
| Memória privada agregada | 1.098,2 MiB | 1.078,3 MiB |
| Memória gráfica compartilhada | 644,6 MiB | 640,9 MiB |
| Processos na árvore | 9 | 8 |
| Janelas depois da queda do host | 1 | 0 |

Nenhuma das 960 amostras combinadas de Pan e Zoom ultrapassou 16,67 ms. O pior
frame foi 8,1 ms em A e 11,1 ms em B. A diferença das médias nesta coleta é
pequena e não indica uma limitação de interação em qualquer alternativa.

A Exportação produziu `7087 × 3543 px`, leu 22,6 MiB de dois originais e gerou
um PNG de 27,0 MiB. As duas alternativas retornaram o mesmo SHA-256:
`e2a0d05bde9ffdac7e1e5ec62732e66158a2d02f12f24745bf671fb299ab26de`.
Isso confirma, para o corte medido, que a topologia não altera a composição nem
desvia a saída para o Cache.

B usou 67,9 MiB a menos de working set e identificou as duas Janelas mais cedo.
A preservou a outra Janela quando um host caiu. O tempo de Cache e o tempo de
Exportação ficaram próximos. Uma coleta única, sempre na ordem A depois B, não
é base suficiente para decidir a topologia; são necessárias repetições com
ordem alternada e os gates de recuperação.

## Falhas encontradas pelo ensaio

A primeira execução de produção revelou que o PixiJS não concluía a
inicialização sob a política de segurança da Janela. O runtime passou a instalar
os sincronizadores estáticos próprios para CSP e a evitar o probe de worker por
`blob`, mantendo `unsafe-eval` proibido.

Depois disso, os logs correlacionados mostraram uma corrida: o probe começava
quando a primeira textura ficava pronta e outra carga podia rematerializar a
Lâmina durante a animação. O `ViewportTexturePool` agora expõe um estado de
estabilização e o Canvas só fornece o alvo do probe depois que todas as texturas
visíveis carregaram ou falharam. O caminho de falha também notifica a mudança
de estado.

O agregador PowerShell ainda revelou uma diferença entre um dicionário ordenado
e um objeto com propriedades. O conversor passou a declarar e devolver
`PSCustomObject`, eliminando a dependência do adaptador implícito do shell.

As três falhas ganharam verificações automatizadas. O ciclo completo em WebView2
de produção é a prova integrada de que inicialização, texturas, Pan, Zoom e
Exportação chegam aos eventos finais.

## Conclusão do gate

Fica atendido o critério do ticket 01 que exige abrir dois Projetos e medir, nas
duas alternativas, abertura, Canvas, Pan/Zoom, memória, memória gráfica,
processos, Cache e Exportação.

Isto não encerra o spike nem escolhe a topologia. O relatório ainda declara como
não medidos:

- recuperação persistida;
- complexidade operacional da IPC.

O próximo gate deve injetar quedas do Processador durante Cache e Exportação,
verificar reinício e publicação incompleta, e medir o custo de recuperação e
coordenação. A aceitação final também deve repetir as amostras alternando a
ordem das topologias.

## Repetição

Executar build e coleta completas:

```powershell
npm run spike:topology
```

Repetir com a build isolada e validada:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Measure-TopologySpike.ps1 -SkipBuild
```
