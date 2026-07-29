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
usado: 172 Fotos JPEG de dois Álbuns, totalizando 1.401,0 MiB. A coleta foi
refeita depois de uma revisão do instrumento invalidar os tempos anteriores de
Pan/Zoom e a sobreposição anterior entre interação e Exportação.

Pan e Zoom foram exercitados no Canvas PixiJS sobre representações reais do
Cache e medidos desde antes da aplicação da transformação até depois do render
correspondente do PixiJS. A Exportação abriu os originais vinculados, validou
tamanho e SHA-256 e renderizou a primeira Lâmina do Projeto principal a 300 DPI.
As duas topologias produziram o mesmo PNG, byte por byte.

Os dados brutos estão em
[0003-topology-interaction-export.json](artifacts/0003-topology-interaction-export.json).
O [resumo gerado](artifacts/0003-topology-interaction-export.md) deriva do
mesmo objeto e concentra os valores da coleta.

## Método

O comando `npm run spike:topology` executou uma build `release`, registrou o
commit, o digest das entradas e os hashes do host e do Processador de Imagens.
O manifesto confirma que os insumos da build estavam limpos e que o checkout
medido corresponde ao executável. A árvore tinha alterações documentais
alheias, mas nenhuma entrada da build diferia do commit
`8e8fe84a6d7e78a01b931b8b22853676c61870ce`.

Para cada alternativa, o instrumento:

1. limpou somente os namespaces descartáveis dos dois Projetos;
2. iniciou os dois hosts de A antes de aguardar qualquer Janela, usando o mesmo
   marco inicial empregado pelo host de B;
3. abriu os dois Projetos e reconstruiu uma representação JPEG de até 1600 px
   para cada Foto;
4. registrou separadamente Janelas prontas, Cache pronto e Canvas pronto; este
   último exige mídia preparada, todas as texturas desejadas estabilizadas e um
   alvo efetivamente apoiado por textura;
5. manteve fechado um gate comum às duas Janelas até ambos os caches
   terminarem;
6. aqueceu cada Canvas por 24 frames;
7. mediu separadamente 120 frames de Pan e 120 frames de Zoom por Projeto,
   iniciando o cronômetro antes da transformação e encerrando-o em um callback
   do ticker executado depois do render do PixiJS;
8. recebeu a conclusão dos dois probes antes de abrir um segundo gate;
9. somente depois desse segundo gate, exportou a primeira Lâmina do Projeto
   principal a 300 DPI, por snapshot validado e a partir de dois originais;
10. amostrou a árvore de processos e os contadores gráficos;
11. forçou a queda do host somente depois de validar o caminho do executável;
12. recalculou o digest do corpus antes de publicar o relatório.

Pan e Zoom usam a mesma transformação de prévia das interações reais, mas o
probe não consolida comandos nem cria entradas no Histórico. As duas operações
foram separadas para que uma não contaminasse as amostras da outra.
O alvo é escolhido pela ordem canônica de composição, independentemente da
ordem assíncrona em que as texturas terminam de carregar. O coletor rejeita a
comparação se as topologias não medirem o mesmo `frameId`; nesta rodada, ambos
os Projetos usaram `frame-01-a` em A e B.

A reconstrução é fria em relação ao Cache descartável do aplicativo. A ordem
permaneceu A seguida de B, portanto o Cache do sistema operacional e o estado
térmico do computador podem favorecer B. Os tempos de Cache, abertura,
interação e Exportação desta rodada única são evidência de viabilidade e de
problemas a investigar, não uma estimativa final de vantagem entre topologias.

## Resultado

| Medida | A — hosts independentes | B — host multiwindow |
|---|---:|---:|
| Duas Janelas identificadas | 1.311 ms | 383 ms |
| Duas Janelas com Cache pronto | 26.120 ms | 24.500 ms |
| Dois Canvas com texturas prontos | 26.396 ms | 24.725 ms |
| Cache frio do aplicativo | 24.118 ms | 23.546 ms |
| Vazão dos originais | 58,1 MiB/s | 59,5 MiB/s |
| Pan, média ponderada | 9,488 ms | 8,826 ms |
| Pan, pior p95 | 18,0 ms | 12,8 ms |
| Pan, frames acima de 16,67 ms | 17 de 240 | 2 de 240 |
| Pan, frames acima de 33,33 ms | 0 de 240 | 0 de 240 |
| Zoom, média ponderada | 8,749 ms | 8,519 ms |
| Zoom, pior p95 | 11,7 ms | 11,9 ms |
| Zoom, frames acima de 16,67 ms | 7 de 240 | 1 de 240 |
| Zoom, frames acima de 33,33 ms | 0 de 240 | 0 de 240 |
| Exportação a 300 DPI | 1.299 ms | 1.241 ms |
| Working set agregado | 1.795,9 MiB | 1.450,9 MiB |
| Memória privada agregada | 1.250,1 MiB | 1.075,7 MiB |
| Memória gráfica compartilhada | 651,8 MiB | 625,4 MiB |
| Processos na árvore | 14 | 8 |
| Janelas depois da queda do host | 1 | 0 |

Nos hosts independentes, 24 das 480 amostras combinadas de Pan e Zoom
ultrapassaram 16,67 ms e nenhuma ultrapassou 33,33 ms. No host multiwindow,
3 das 480 amostras ultrapassaram 16,67 ms e nenhuma ultrapassou 33,33 ms. O
pior frame foi 32,6 ms em A e 18,3 ms em B.

Nesta execução simultânea, B teve menor média ponderada e menor pior p95 no
Pan; no Zoom, as médias e os piores p95 ficaram próximos. O resultado precisa
ser reproduzido e ainda não isola topologia de ordem, estado térmico, WebView2
ou contenção do sistema.

A Exportação produziu `7087 × 3543 px`, leu 22,6 MiB de dois originais e gerou
um PNG de 27,0 MiB. As duas alternativas retornaram o mesmo SHA-256:
`e2a0d05bde9ffdac7e1e5ec62732e66158a2d02f12f24745bf671fb299ab26de`.
Isso confirma, para o corte medido, que a topologia não altera a composição nem
desvia a saída para o Cache. Como o segundo gate eliminou a sobreposição com os
probes, os 1.299 ms de A e 1.241 ms de B são amostras de Exportação isoladas das
interações; uma amostra por alternativa ainda não permite atribuir causalidade.

B usou 345,0 MiB a menos de working set, 174,4 MiB a menos de memória privada e
26,4 MiB a menos de memória gráfica compartilhada. A preservou a outra Janela
quando um host caiu. Uma coleta única, sempre na ordem A depois B, não é base
suficiente para decidir a topologia; são necessárias repetições com ordem
alternada e os gates de recuperação.

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

Uma revisão posterior encontrou quatro defeitos metodológicos: o cronômetro
começava depois da transformação, um `requestAnimationFrame` não provava que o
PixiJS já havia renderizado, a Exportação do Projeto principal podia sobrepor o
probe da outra Janela e os hosts de A eram aguardados sequencialmente. O alvo do
probe agora fornece o próximo frame efetivamente renderizado; há um evento de
Canvas pronto por Projeto, um segundo gate exclusivo para Exportação e os dois
hosts de A são iniciados antes de qualquer espera.

O estado do alvo também passou a distinguir `pending`, `ready` e falha terminal
de textura, evitando converter uma falha real em timeout. O Processador de
Imagens devolve estágios seguros e tipados para verificação da fonte,
decodificação, composição, preparação, codificação, publicação e verificação
da saída. Testes cobrem fonte alterada, JPEG inválido, destino indisponível e
substituição de uma Exportação existente.

Ao executar os dois hosts de A simultaneamente, o ensaio ainda encontrou uma
colisão no diretório padrão de dados do WebView2: os dois processos iniciavam,
mas somente uma interface carregava. A criação das Janelas passou a ser
explícita e `AppPaths` agora deriva diretórios sob
`%LOCALAPPDATA%\MyAlbuns2\State\WebView2`. Cada host independente recebe um
namespace próprio; as Janelas de B compartilham deliberadamente o namespace do
mesmo host. O coletor final abriu os dois hosts de A em paralelo.

Essas falhas ganharam verificações automatizadas. O ciclo completo em WebView2
de produção é a prova integrada de que inicialização, texturas, Pan, Zoom,
barreiras e Exportação chegam aos eventos finais.

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
