---
status: current
document: research
date: 2026-09-30
platform: windows-11-x64
---

# Desempenho do Painel de Layouts

O ensaio mede, na versão local, o tempo entre o clique no controle de Layouts
da Barra da Lâmina e o Painel com todas as miniaturas prontas. Compara a `main`
em `00606222`, com a versão 1 do Gerador e até dez sugestões, com a versão 2 do
[ADR 0013](../adr/0013-diversificar-e-harmonizar-as-sugestoes-do-gerador.md),
com até vinte, antes e depois de otimizar o núcleo no perfil de
desenvolvimento. Depois, compara a sessão antes e depois de reaproveitar
buscas repetidas e o Gerador antes e depois de buscar as divisões em
paralelo.

## Resultado

Com `myalbuns-core` compilado em `opt-level = 2` também no perfil `dev`, o
Painel de uma Lâmina com 20 Frames passou de **1.059 ms para 261 ms**, cerca
de **4 vezes mais rápido**. A busca do Gerador caiu de 979 para 195 ms. As
sugestões, a ordem e as prévias continuam as mesmas.

Mediana de seis aberturas do Painel, com o tempo da busca entre parênteses:

| Frames na Lâmina | `main` (versão 1) | Versão 2 | Versão 2, núcleo em nível 2 |
| ---: | ---: | ---: | ---: |
| 4 | 78 ms (5) | 92 ms (6) | 83 ms (3) |
| 8 | 167 ms (109) | 216 ms (147) | 107 ms (37) |
| 12 | 371 ms (311) | 403 ms (336) | 150 ms (79) |
| 16 | 713 ms (648) | 739 ms (665) | 211 ms (140) |
| 20 | 1.050 ms (984) | 1.059 ms (979) | 261 ms (195) |

Os números valem para esta máquina e este Projeto de teste. Não são uma
promessa de tempo para qualquer Lâmina.

## Onde está o tempo

Antes da mudança, a busca (`query_layouts`) era 93% do tempo com 20 Frames;
depois, 75%. As prévias (`preview_layout`) são pedidas em paralelo, uma por
sugestão: as vinte terminam em 25 a 27 ms, e em cerca de 20 ms depois da
mudança. O restante, de 40 a 60 ms, é a montagem do Painel e o IPC, e não
depende do Gerador.

Um executável que chama as mesmas operações pela API pública do
`ProjectCore`, sem o aplicativo, mostra o efeito do perfil na busca com
20 Frames:

| Perfil do núcleo | Versão 1 | Versão 2 |
| --- | ---: | ---: |
| `dev`, nível 0 (anterior) | 1.006 ms | 971 ms |
| `dev`, nível 1 | — | 335 ms |
| `dev`, nível 2 (escolhido) | — | 190 ms |
| `dev`, nível 3 | — | 185 ms |
| `release` (nível 3, ThinLTO, uma unidade de geração de código) | 232 ms | 169 ms |

A busca no aplicativo (195 ms) coincide com a do executável isolado (190 ms).
Nos dois casos só o núcleo muda de nível: o executável continua em nível 0 e o
`myalbuns-desktop` em nível 1. O trabalho do Gerador é, portanto, compilado no
próprio `myalbuns-core`, e não em operações genéricas instanciadas por quem o
chama.

## Mudança

O manifesto da raiz passa a otimizar o núcleo no perfil de desenvolvimento:

```toml
[profile.dev.package.myalbuns-core]
opt-level = 2
```

O nível 1 ainda deixa a busca de 20 Frames em 335 ms. O nível 2 fica a 12% do
build de release, e o nível 3 ganha só mais 3%. O perfil release não muda.

O ajuste vale para todos os alvos do pacote, inclusive seus testes. Custo
medido nesta máquina depois de alterar um arquivo do núcleo, em duas rodadas:

| Operação | Nível 0 | Nível 2 |
| --- | ---: | ---: |
| Recompilar o núcleo | 4,4 a 5,6 s | 4,6 a 8,8 s |
| Recompilar o núcleo e seus testes | 16,3 a 16,5 s | 25,9 a 28,3 s |
| Rodar os testes do núcleo já compilados | 55,4 a 56,1 s | 49,4 a 49,5 s |

Alterar o núcleo e rodar seus testes leva quase o mesmo tempo: cerca de 72 s
antes e 76 s depois. Os pacotes que dependem do núcleo, como o
`myalbuns-desktop`, continuam no nível que já tinham.

As verificações de depuração e de overflow continuam herdadas do perfil `dev`.
Código otimizado é mais difícil de acompanhar passo a passo num depurador; para
isso, basta compilar com
`--config 'profile.dev.package.myalbuns-core.opt-level=0'`.

## Buscas repetidas na mesma sessão

Com o Painel aberto, a busca é refeita sempre que a revisão do Projeto muda,
como depois de aplicar uma sugestão, e também a cada reabertura. Nesses casos
a consulta costuma ser a mesma: aplicar um Layout muda as posições, e não a
quantidade, a orientação ou a ordem dos Frames. A `ProjectSession` passou a
guardar, só em memória, as gerações das últimas 32 consultas do Projeto aberto
e a reaproveitar a geração de uma consulta igual. A consulta inteira é a
chave. Último Layout, favoritos, Layouts personalizados e Mapeamento continuam
resolvidos a cada vez. O contrato está no
[design 0026](../design/0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md).

Mediana do tempo até o Painel pronto, sem e com o reaproveitamento, já com o
núcleo em nível 2:

| Frames na Lâmina | Primeira abertura | Reabrir o Painel | Aplicar com o Painel aberto |
| ---: | ---: | ---: | ---: |
| 4 | 116 → 113 ms | 73 → 76 ms | 75 → 72 ms |
| 8 | 108 → 106 ms | 104 → 87 ms | 104 → 73 ms |
| 12 | 140 → 154 ms | 138 → 87 ms | 137 → 72 ms |
| 16 | 197 → 209 ms | 201 → 87 ms | 201 → 72 ms |
| 20 | 258 → 263 ms | 256 → 96 ms | 267 → 74 ms |

Com 20 Frames, a busca repetida caiu de 194 a 196 ms para 2 ms, que são o IPC
e a serialização da lista. No executável isolado, a primeira busca leva
180 ms e a repetida, 0,06 ms. A primeira abertura não muda: as diferenças
dessa coluna ficam dentro da variação entre rodadas.

Continuam executando o Gerador a primeira consulta de cada composição, a
organização automática ao adicionar uma Foto, porque a quantidade de Frames
muda, e cada quantidade nova escolhida no seletor do Painel. Cada geração tem
no máximo vinte sugestões; as 32 somam, no pior caso, algumas centenas de
kilobytes.

## Divisões buscadas em paralelo

A busca por Página tenta cada divisão dos Frames entre as duas Páginas, e os
grupos complementares tentam cada divisão da Lâmina em dois grupos. Com
10 verticais e 10 horizontais são 119 divisões por Página e 114 em grupos, e
cada uma é independente das outras. Antes da mudança, no build de release,
elas levavam 70 ms e 39 ms dos 168 ms da busca.

O Gerador passou a buscar essas divisões em paralelo, com as threads da
biblioteca padrão do Rust, sem dependência nova. Cada processador disponível
pega a próxima divisão livre, e os resultados entram na ordem da busca
sequencial. Onde não há threads, como na página de comparação em WebAssembly,
a busca continua sequencial.

Busca no executável isolado, perfil `dev` com o núcleo em nível 2, fixado nos
doze processadores lógicos dos núcleos de desempenho, mediana de três rodadas
intercaladas:

| Frames | Sequencial | Em paralelo |
| ---: | ---: | ---: |
| 3 | 0,3 ms | 0,5 ms |
| 8 | 33 ms | 12,5 ms |
| 12 | 73 ms | 29 ms |
| 16 | 147 ms | 60 ms |
| 20 | 190 ms | 96 ms |
| 30 | 245 ms | 74 ms |

Com poucos Frames, abrir as threads custa cerca de 0,2 ms a mais. Com 20 Frames,
o que restou no build de release é, sobretudo, trabalho feito candidato a
candidato depois das divisões: seleção final (34 ms), validação e quantização
(12 ms) e chaves de espelhamento (8 ms). As divisões por Página caíram de 70
para 16 ms, e os grupos, de 39 para 8 ms.

No aplicativo de desenvolvimento, só a primeira abertura do Painel executa o
Gerador desde o reaproveitamento das buscas. Valores das duas rodadas
intercaladas de cada versão, com a busca entre parênteses:

| Frames | Sequencial | Em paralelo |
| ---: | ---: | ---: |
| 4 | 114 e 170 ms (6) | 119 e 132 ms (4 a 9) |
| 8 | 131 e 153 ms (48 a 55) | 91 e 113 ms (17 a 18) |
| 12 | 186 e 235 ms (98 a 112) | 102 e 111 ms (34 a 35) |
| 16 | 229 e 290 ms (155 a 194) | 147 e 181 ms (71 a 100) |
| 20 | 297 e 339 ms (212 a 241) | 173 e 222 ms (98 a 116) |

As rodadas começaram com a CPU livre, mas outra atividade da máquina voltou
durante elas; por isso os totais ficaram acima dos da seção anterior e variam
entre rodadas. A busca, medida no IPC, é o número mais estável: com 20 Frames,
cai para cerca da metade. Reabrir e aplicar com o Painel aberto não passam
pelo Gerador e ficaram na mesma faixa nas duas versões.

## Trabalho por candidata depois das divisões

Com as divisões em paralelo, sobraram três etapas que percorrem as cerca de
3.100 composições de uma consulta com 20 Frames:

- **Validação e quantização.** Confere margens, lado mínimo, proporções e o
  intervalo entre cada par de Frames, converte para micrômetros e monta a
  chave usada para descartar duplicadas. Cada composição passou a ser
  conferida em paralelo; as duplicadas continuam sendo descartadas na ordem
  em que as famílias as produziram.
- **Chaves de espelhamento.** Cada composição calcula até 16 formas espelhadas
  e guarda a menor. Também passou a ser calculada em paralelo.
- **Seleção final.** Escolhe até vinte sugestões, cada vez a de maior nota entre
  qualidade e novidade. Antes, depois de cada escolha, a novidade de todas as
  candidatas era recalculada. Como a novidade só diminui, o valor guardado é um
  teto: a seleção passou a examinar as candidatas do maior teto para o menor e
  a recalcular só quem ainda pode vencer, com o mesmo desempate pela posição
  no ranking. Cada comparação também deixou de arredondar a sobreposição de
  pares de Frames que nem se tocam, cuja sobreposição é exatamente zero. Com
  20 Frames, os cálculos de novidade caíram de cerca de 50 mil para 14 mil.

Recalcular a novidade em paralelo a cada rodada foi testado e descartado:
abrir as threads vinte vezes por consulta custava mais do que poupava com 12
Frames ou menos. Pelo mesmo motivo, uma thread extra só é aberta para cada
quatro divisões ou 64 composições; listas menores ficam na thread que chamou.
Sem esse limite, a consulta de 3 Frames passava de 0,5 para 2 ms.

Etapas da busca de 20 Frames no build de release, menor valor de cinco
execuções com a máquina carregada por outra sessão:

| Etapa | Antes | Depois |
| --- | ---: | ---: |
| Seleção final | 34 ms | 11 ms |
| Validação e quantização | 12 ms | 3,5 ms |
| Chaves de espelhamento | 8 ms | 1,6 ms |
| Busca inteira | 83 ms | 41 ms |

Busca no executável isolado, nas mesmas condições da seção anterior, mediana
de três rodadas intercaladas com a CPU livre:

| Frames | Original | Divisões em paralelo | Também por candidata |
| ---: | ---: | ---: | ---: |
| 3 | 0,3 ms | 0,5 ms | 0,2 ms |
| 8 | 33 ms | 12,4 ms | 10,9 ms |
| 12 | 73 ms | 28,5 ms | 20,4 ms |
| 16 | 147 ms | 55,6 ms | 34 ms |
| 20 | 190 ms | 89,7 ms | 46,6 ms |
| 30 | 245 ms | 70,3 ms | 48,8 ms |

A primeira coluna vem da seção anterior; as outras duas foram medidas juntas.
Na matriz de 1.150 consultas, a soma do Gerador caiu de 38,5 s, na versão
sequencial, para 15,3 s com as divisões em paralelo e 9,2 s com esta etapa, com
as mesmas respostas.

No aplicativo de desenvolvimento, primeira abertura do Painel nas duas rodadas
intercaladas de cada versão, com a busca entre parênteses:

| Frames | Divisões em paralelo | Também por candidata |
| ---: | ---: | ---: |
| 8 | 124 e 114 ms (18 a 20) | 111 e 119 ms (15 a 16) |
| 12 | 145 e 145 ms (38 a 44) | 133 e 132 ms (29 a 31) |
| 16 | 147 e 157 ms (64 a 66) | 132 e 139 ms (44 a 45) |
| 20 | 196 e 192 ms (105 a 110) | 142 e 160 ms (60 a 65) |

Com 4 Frames, a busca leva poucos milissegundos nas duas versões, e a
diferença no total é variação entre rodadas. Reabrir e aplicar com o Painel
aberto continuaram entre 80 e 130 ms nas duas versões, porque não passam pelo
Gerador. Uma primeira tentativa desta medição foi descartada: um processo de
medição anterior, que deveria ter sido encerrado, abriu o aplicativo ao mesmo
tempo na mesma porta de depuração.

## Método

- Máquina: Windows 11, Intel Core i5-13450HX, 6 núcleos de desempenho com
  dois processadores lógicos cada e 4 núcleos de eficiência. Rust 1.98.0.
- Aplicativo: `Invoke-MyAlbunsTauriBuild --debug --no-bundle`, com o Rust no
  perfil `dev`. A compilação fica fora das medições.
- Projeto de teste: Lâmina de 60 × 30 cm com Páginas de 30 × 30 cm, Margem de
  15 mm, Intervalo de 5 mm, lado mínimo de 20 mm e Permissão de Páginas e
  Lâmina. Cinco Lâminas com 4, 8, 12, 16 e 20 Frames, alternando verticais de
  10 × 15 cm e horizontais de 15 × 10 cm, com JPEGs sintéticos de
  1.200 × 1.800 e 1.800 × 1.200 pixels.
- Medição: um script conecta ao WebView2 do Host pela porta de depuração,
  clica em "Layouts da lâmina NN" e espera todas as miniaturas habilitadas e
  dois quadros de animação. Os tempos de IPC vêm do Resource Timing
  (`ipc.localhost/query_layouts` e `ipc.localhost/preview_layout`). São três
  cliques por Lâmina, fechando o Painel pelo mesmo controle, em duas aberturas
  do aplicativo.
- CPU: o processo do Host fica nos núcleos de desempenho (afinidade `0xFFF`),
  com prioridade alta. Sem isso, a mesma medição variou 2×, conforme o
  escalonador movia o Gerador entre núcleos de desempenho e de eficiência. O
  executável isolado usa a afinidade `0x3`; cada valor é a mediana de cinco
  execuções e, com mais de uma rodada, a mediana das rodadas.
- Janela visível: coberta por outras janelas, a página do WebView2 fica com
  `visibilityState` `hidden` e para de desenhar. Uma tentativa travou assim: o
  Painel não abriu e um menu de contexto da Lâmina ficou sobre o controle. O
  script mantém a janela do Host acima das outras durante a medição e a torna
  transparente ao mouse real: o cursor parado sobre ela chegou a destacar uma
  sugestão e impedir o fechamento do Painel.
- Buscas repetidas: a primeira abertura de cada Lâmina é o primeiro clique; as
  reaberturas são o segundo e o terceiro. Para aplicar, o script abre o Painel
  logo depois das aberturas da mesma Lâmina, clica três vezes na segunda
  sugestão e mede até o Painel ficar pronto para a nova revisão. As duas
  versões foram medidas de forma intercalada, duas rodadas cada, somente com a
  CPU livre de compilações: uma compilação de outro projeto na mesma máquina
  chegou a dobrar os tempos numa primeira tentativa. Na comparação das
  divisões em paralelo, cada rodada também esperou o uso da CPU ficar abaixo
  de 25%.

## Verificação

- As 1.150 consultas da análise do
  [ADR 0013](../adr/0013-diversificar-e-harmonizar-as-sugestoes-do-gerador.md)
  devolvem os mesmos resultados, sugestão por sugestão, compiladas em nível 0 e
  em nível 2. O Gerador levou 190,6 s e 38,5 s para percorrer a matriz inteira.
- `npm run test:rust` passou com o núcleo em nível 2 e o reaproveitamento das
  buscas: 1.092 testes, inclusive os que exportam pelo Processador real. Os corpora visuais
  `layout-panel-cases.json`, `sheet-duplication-cases.json` e
  `frame-clipboard-cases.json`, produzidos antes em nível 0, não foram
  regenerados.
- `cargo fmt --all -- --check` e `cargo clippy --workspace --all-targets -- -D warnings`
  passaram.
- Testes do reaproveitamento: uma consulta igual devolve a mesma geração e uma
  Margem diferente executa o Gerador; a consulta usada há mais tempo sai
  primeiro quando a lista passa de 32; e a consulta repetida depois de aplicar
  uma sugestão devolve a mesma lista que uma sessão nova sobre a mesma revisão.
  O corpus visual do Painel continuou igual sem regeneração.
- Divisões em paralelo: as 1.150 consultas devolvem os mesmos resultados da
  busca sequencial; a matriz inteira caiu de 38,5 s para 15,3 s sem fixar a
  afinidade. Os testes do executor conferem a ordem com trabalhos de duração
  desigual, entradas vazias e a propagação de uma falha. A página em
  WebAssembly continuou gerando as sugestões, pelo caminho sequencial.
  `npm run test:rust` passou com 1.095 testes, e o clippy sem avisos.
- Trabalho por candidata: as 1.150 consultas continuam idênticas às da versão
  sequencial original. Os testes do executor passaram a conferir também a
  alteração de cada item no lugar e que listas curtas ficam na thread que
  chamou. A página em WebAssembly gera as vinte sugestões de 20 Frames em
  222 ms, contra 390 ms antes, pelo caminho sequencial. `npm run test:rust`
  passou com 1.097 testes, e o clippy sem avisos.

Scripts, Projeto de teste e resultados ficam somente na cópia local, em
`.scratch/planos/2026-09-30-layouts-mais-bonitos-e-harmonicos/laboratorio/desempenho/`.
