---
status: current
document: research
date: 2026-10-13
platform: windows-11-x64
---

# Progresso de abertura em computador fraco

Um usuário com um computador antigo (provavelmente dois núcleos, 4 GB de RAM e
disco mecânico) relatou travamentos e lentidão. Para reproduzir essas condições,
o MyAlbuns instalado foi medido numa máquina virtual Hyper-V com recursos
limitados. A medição mostrou que, na primeira abertura depois de ligar o
computador, o diálogo “Abrindo projeto” às vezes não aparece. Quando isso
acontece, nada fica na tela por minutos. Esta pesquisa explica a causa e
registra a decisão de desenhar esse progresso como uma janela nativa do Windows.

## Como foi medido

- O instalador da versão 0.1.0 foi instalado de forma silenciosa numa máquina
  virtual Windows 11 Pro limpa, com a GPU Intel do hospedeiro compartilhada por
  GPU-P. O disco é restaurado a cada rodada.
- Perfil `cliente-estimado`: 2 processadores virtuais limitados a 25%, 4 GB fixos
  e discos limitados a 3000 IOPS (cerca de 24 MB/s). Perfil `fraco`: 2
  processadores a 40%, 4 GB e 5000 IOPS. O limite de processador desses perfis
  não chegou a ser aplicado; ver Limites.
- Projeto de teste com 300 fotos JPEG de 6000 × 4000 e 6720 × 4480 pixels (cerca
  de 10 MB cada) e 40 Lâminas.
- Cada rodada reinicia a máquina virtual. A primeira abertura é fria: disco e
  WebView2 sem cache do sistema. A segunda reabre o mesmo Projeto.
- Os tempos vêm dos logs JSONL do aplicativo, medidos a partir do lançamento.
  Contadores de processo foram lidos a cada segundo, com uso de processador,
  memória e disco por papel: Host, WebView2 e Processador de Imagens.
- As rodadas e o laboratório ficam fora do Git.

## O que acontece numa abertura fria

| Rodada | Processo iniciou | WebView principal pronto | Diálogo pronto | WebView do Projeto pronto |
|---|---|---|---|---|
| `fraco` | 2,8 s | 9,6 s | 10,8 s | 13,6 s |
| `cliente-estimado` | 3,8 s | 9,4 s | 11,0 s | 22,4 s |
| `cliente-estimado` | 3,1 s | 18,9 s | não ficou pronto (aviso em 24,1 s) | 28,2 s |

Na segunda abertura, cada etapa leva cerca de 1 s.

O processo começa em 3 s, mas a primeira coisa visível aparece, no melhor caso,
aos 11 s. A causa é uma cadeia de inicializações do WebView2:

1. **Verificação gráfica.** O processo global cria primeiro a janela WebView da
   tela de boas-vindas, oculta, só para verificar o suporte gráfico, e espera
   até 10 s por essa verificação (`complete_graphics_gate`). Essa janela é o
   primeiro WebView2 iniciado a frio e levou de 6 a 16 s.
2. **Navegador próprio do diálogo.** Só depois disso o diálogo é criado. Ele usa
   uma pasta de dados própria (`global-progress`) e o argumento `--disable-gpu`,
   diferente das outras janelas. Por isso o WebView2 inicia para ele uma segunda
   árvore inteira de processos (navegador, GPU, renderizador e utilitários).
3. **Prazo de 5 s.** O diálogo tem 5 s para carregar a página
   (`DIALOG_LOAD_TIMEOUT`). Com o disco frio e o Host do Projeto iniciando ao
   mesmo tempo, esse prazo às vezes não basta. Nesse caso o diálogo é destruído
   e a abertura segue sem nenhuma janela (`project_launch_progress_dialog_unavailable`).
4. **Janela do Projeto oculta.** A Janela do Projeto inicia oculta e só aparece
   quando todas as imagens estão preparadas ([0010](../design/0010-armazenamento-local-e-cache.md)).
   Com 300 fotos nesse perfil, são cerca de três minutos sem nada na tela.

Durante a abertura, o processo global e o Host mantêm três árvores de processos
do WebView2 ao mesmo tempo: `global`, `global-progress` e `project-N`. Os
diálogos do Host e o visualizador de imagens usam uma quarta pasta, a padrão do
Tauri. Essa última conclusão vem do código e não foi observada.

## O que diz a documentação

- **Não usar WebView2 para a interface inicial.** O
  [guia de desempenho do WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance)
  recomenda desenhar telas de abertura e diálogos simples com janelas nativas
  leves, por causa do custo de iniciar o WebView2 e da disputa por recursos.
  Na partida a frio, o WebView2 precisa iniciar seus processos e caches de
  disco, e o tempo varia com o hardware.
- **Nenhum tempo esperado.** Segundo uma
  [resposta da Microsoft](https://learn.microsoft.com/en-us/answers/questions/1185053/msedgewebview2-show-slowly-first-time-in-cold-star),
  não existe tempo de inicialização esperado; depende da máquina. Um prazo fixo
  de 5 s não tem base.
- **Cada pasta de dados é um navegador.** Pela página de
  [modelo de processos](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model),
  WebViews com a mesma pasta e as mesmas opções compartilham um navegador.
  Cada pasta diferente inicia uma árvore própria. A página de
  [pastas de dados](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder)
  pede para evitar várias pastas ao mesmo tempo.
- **Opções diferentes, navegador diferente.** O WRY cria um ambiente por
  WebView. Pela documentação do WRY, argumentos de navegador diferentes exigem
  pastas de dados diferentes, e duas opções diferentes na mesma pasta impedem a
  criação do segundo WebView (`ERROR_INVALID_STATE`).
- **Relatos de terceiros.** Há relatos de partidas a frio de 7 a 15 s em máquinas
  comuns. Num deles
  ([WebView2Feedback #1818](https://github.com/MicrosoftEdge/WebView2Feedback/issues/1818)),
  a demora vinha da descoberta automática de proxy (WPAD). A máquina virtual
  estava sem rede; num computador com rede essa espera pode se somar.

[A pesquisa do diálogo preto](2026-09-13-webview2-dialogo-preto.md) já indicava
uma janela Win32 para o progresso simples. O isolamento do `global-progress`
com `--disable-gpu` foi criado contra o diálogo preto, depois atribuído ao
RivaTuner ([diagnóstico](2026-09-13-rivatuner-webview2-configuracoes.md)). A
proteção continua útil contra ganchos de terceiros, mas custa uma partida a frio
a mais em toda abertura.

## Conclusão

O diálogo não deve depender do WebView2 para aparecer. O progresso de abertura,
criação e processamento de imagens passa a ser uma janela nativa do Windows:

- é criada pelo processo global antes de qualquer WebView, logo que ele inicia
  com arquivos para abrir;
- é desenhada com Direct2D em modo de software, sem depender da GPU, com as
  medidas e as cores do diálogo atual;
- não tem prazo para falhar: não há página para carregar;
- elimina a árvore `global-progress` das aberturas.

As decisões de Recuperação e de Cópia externa continuam em páginas WebView.
Elas são raras, aparecem quando o processo global já tem um navegador pronto e
passam a usar a mesma pasta e as mesmas opções da tela de boas-vindas. Por isso
não iniciam outro navegador.

O processo por Projeto ([ADR 0005](../adr/0005-adotar-tauri-react-rust.md))
não é a causa: seu custo aparece em memória, não neste atraso.

Com a janela nativa, na mesma abertura fria do perfil `cliente-estimado`, o
progresso apareceu 2,9 s depois do lançamento. O primeiro WebView ficou pronto
só aos 19,5 s.

## Início da preparação das imagens

Com o progresso visível, a mesma rodada mostrou outra espera: a contagem
“Preparando imagens” só começou aos 24 s. A preparação das imagens não usa
WebView nem GPU, mas esperava por eles:

| Etapa | Momento |
|---|---|
| Janela nativa de progresso | 2,9 s |
| Tela de boas-vindas termina a verificação gráfica; o Host do Projeto é iniciado | 19,5 s |
| WebView do Projeto pronto | 22,9 s |
| A página do Projeto pede a preparação; “0 de 300” | cerca de 24 s |

Duas esperas não eram necessárias:

1. **O Host esperava a verificação gráfica.** O processo global só iniciava o
   Host depois que a tela de boas-vindas, um WebView oculto, terminava a
   verificação de WebGL2 (`complete_graphics_gate`). Essa verificação existe
   para que o editor não abra sem WebGL2
   ([ADR 0005](../adr/0005-adotar-tauri-react-rust.md)). A página do editor faz
   a mesma verificação e só fica pronta se ela passar. Assim, a verificação
   decide se o editor aparece, não se as fotos podem ser preparadas.
2. **A preparação esperava a página.** O Host já calcula, antes de criar
   qualquer WebView, a lista das fotos a preparar. Mesmo assim, só começava
   quando a página do Projeto montava e pedia a preparação
   (`prepare_project_startup_images`).

Decisão:

- **O Host prepara as imagens assim que inicia.** A página não dispara mais o
  trabalho: o pedido dela se junta à preparação em andamento, que continua
  rodando uma única vez. Os problemas encontrados continuam sendo entregues à
  página. As mudanças de mídia encontradas antes de a página escutar ficam
  guardadas e chegam a ela num único aviso quando ela pede a preparação.
- **Os arquivos abertos pelo Windows iniciam seus Hosts junto com a
  verificação gráfica.** A verificação continua decidindo se a tela de
  boas-vindas aparece. Se ela recusar o WebGL2, os Hosts que ainda não ficaram
  prontos são encerrados sem aviso próprio, e a tela de boas-vindas mostra a
  mensagem da verificação. Se a verificação passar do prazo, esses Hosts
  continuam, porque cada editor faz a própria verificação. Quando um editor
  fica pronto, a verificação é dada como aprovada.

## Resultado com SSD

O computador do usuário tem SSD. O perfil `cliente-ssd` mantém o processador e
a memória do `cliente-estimado` e limita o disco a 64 000 operações por segundo,
cerca de 500 MB/s, como um SSD SATA. A versão publicada (`main`, 597853b9) e
a versão com janela nativa e preparação antecipada foram medidas na mesma
máquina virtual, cada uma com uma abertura fria e uma morna. O tempo vai até a
Janela do projeto aparecer (`project_window_presented`).

| Etapa | Publicada, fria | Nova, fria | Publicada, morna | Nova, morna |
|---|---|---|---|---|
| Progresso na tela | 11,1 s | 2,0 s | 4,6 s | 0,1 s |
| Host do Projeto iniciado | 14,2 s | 8,1 s | 10,2 s | 4,3 s |
| Primeira imagem preparada | 18,8 s | 10,8 s | — | — |
| Janela do projeto apareceu | 150,9 s | 140,3 s | 19,4 s | 7,8 s |

Na abertura morna, com as prévias já no Cache, o editor passou a aparecer em
7,8 s, contra 19,4 s. Na fria, o ganho é de cerca de 10 s: quase todo o tempo
restante é a própria preparação das 300 fotos, cerca de 130 s nas duas versões.

Com SSD, o disco deixa de ser o limite: depois dos primeiros 10 s ele fica
cerca de 10% ocupado. A preparação é que é lenta, por dois motivos:

- **Uma foto por vez.** O número de Processadores de Imagens simultâneos é o
  número de núcleos menos um, para deixar um núcleo ao editor
  (`default_capacity`). Com dois núcleos, as 300 fotos são preparadas uma de
  cada vez, embora o editor ainda não esteja na tela durante a abertura.
- **Um processo por foto.** Cada foto leva cerca de 290 ms de trabalho no
  Processador de Imagens, e entre um processo e o seguinte passam cerca de
  60 ms na mediana, que somaram 36 s numa abertura fria.

Esses dois pontos são o próximo passo para encurtar a abertura nesse
computador. O primeiro foi tratado na seção seguinte.

## Preparação com todos os núcleos durante a abertura

Durante a abertura, a Janela do projeto ainda não está na tela, então o núcleo
reservado ao editor fica sem uso. A regra passou a depender do foco:

- O Host cuja janela não está em uso prepara imagens em segundo plano, com
  prioridade abaixo do normal, e pode usar todos os núcleos. Isso inclui o
  Host que ainda está abrindo e os Hosts de outros Projetos abertos juntos.
- O Host da janela em uso continua com o número de núcleos menos um.
- O limite vale para a máquina toda. Os Processadores de Imagens em segundo
  plano tentam primeiro a vaga extra, a que a janela em uso não usa.

As quatro rodadas abaixo foram feitas em sequência, no perfil `cliente-ssd`,
com abertura fria. Os dois Projetos abertos juntos têm 300 e 200 fotos.

| Abertura | Regra anterior | Todos os núcleos em segundo plano |
|---|---|---|
| 1 Projeto, editor na tela | 121,2 s | 79,9 s |
| 2 Projetos, primeiro editor | 155,2 s | 103,3 s |
| 2 Projetos, segundo editor | 192,3 s | 135,4 s |

Com um Projeto, as 300 fotos ficaram prontas em 68 s, contra 111 s uma de
cada vez. Cada foto continuou levando cerca de 330 ms no Processador de
Imagens, contra 280 ms sozinha. Isso mostra que os dois núcleos somam quase por
inteiro.

Com dois Projetos, o primeiro editor ainda espera parte das fotos do segundo,
porque os dois Hosts dividem as mesmas vagas.

Duas rodadas anteriores, feitas enquanto o hospedeiro estava ocupado, tiveram
cada foto 2 a 3 vezes mais lenta nas duas regras e foram descartadas.

## Um Projeto por vez na abertura

Quando vários Projetos abrem juntos, um prepara as imagens por vez. O Host
pede a vez enquanto a preparação da abertura roda, pega a vez antes do primeiro
Processador de Imagens e a solta quando a preparação termina. A vez vale para
o computador todo. Um Projeto com todas as prévias no Cache não inicia
Processador de Imagens, então não espera. Um editor já aberto também não pede a
vez. A ordem entre os Projetos é a de quem pede primeiro.

As duas rodadas abaixo foram feitas em sequência, no perfil `cliente-ssd`, com
abertura fria dos mesmos dois Projetos, de 300 e 200 fotos.

| Abertura | Dividindo as vagas | Um Projeto por vez |
|---|---|---|
| Primeiro editor | 117,8 s | 65,0 s |
| Segundo editor | 143,8 s | 148,6 s |

O Projeto de 200 fotos foi o primeiro, e o segundo Host esperou 49,4 s pela
vez. O primeiro editor passou a aparecer quase no tempo de um Projeto sozinho.
O segundo ficou praticamente igual.

O segundo não ficou mais rápido por causa da memória, e não dos núcleos.
Depois que o primeiro editor apareceu, sobravam cerca de 1,25 GB livres. O
orçamento de memória guarda 512 MiB e usa metade do resto (`resources.rs`), e
duas fotos de cerca de 200 a 240 MB não cabiam juntas. A admissão segurou o
segundo Processador de Imagens 82 vezes, somando 32,6 s. Nas rodadas que
dividem as vagas, as duas preparações correm antes de haver um editor aberto, e
essa espera somou menos de 6 s. A seção [Orçamento fixo de
memória](#orçamento-fixo-de-memória) mostra que essas esperas não eram o limite
real.

Neste laboratório, as rodadas de dois Projetos variaram cerca de 15 s entre
sessões: o primeiro editor dividindo as vagas apareceu em 103,3 s numa sessão e
em 117,8 s na outra.

## Memória exigida para dois Processadores de Imagens

A estimativa por foto não é o que mais pesa. Em 1300 execuções do Processador
de Imagens nas rodadas `cliente-ssd`, o pico de memória física foi de 105 MiB
para as fotos de 24 MP e de 125 MiB para as de 30 MP. A estimativa de admissão
dessas fotos é de 189 e 230 MiB, cerca de 1,8 vez o pico. Isso confirma a
calibração de [Memória do Processador por formato](2026-09-25-memoria-do-processador.md).

Pela [revisão da admissão por memória](../design/0032-revisao-da-admissao-por-memoria.md),
um segundo trabalho simultâneo só começa se a estimativa dele couber na metade
da RAM livre acima da margem, descontadas as reservas ativas. Num computador de
4 GB, a margem é de 512 MiB. A RAM livre lida já desconta a memória que o
primeiro Processador de Imagens ocupa, e a reserva dele continua descontada
inteira. Para dois Processadores de Imagens ao mesmo tempo, a regra exige:

| Fotos | RAM livre exigida | Memória que os dois usam |
|---|---:|---:|
| 2 × 24 MP | 1268 MiB | cerca de 210 MiB |
| 24 MP + 30 MP | 1350 MiB | cerca de 230 MiB |
| 2 × 30 MP | 1432 MiB | cerca de 250 MiB |

Na abertura de dois Projetos, um por vez, a RAM livre ficou entre 926 e
1450 MiB durante a preparação. Com o primeiro editor aberto, a mediana foi de
cerca de 1250 MiB. Por isso o segundo Processador de Imagens esperou 82 vezes.
Num Projeto aberto sozinho, a RAM livre ficou perto de 1400 MiB, e a espera
somou 1,2 s.

Sem a divisão por dois, mantendo a margem de 512 MiB e a estimativa atual, o
limiar cairia para 890 a 972 MiB. Mesmo descontando os dois Processadores de
Imagens inteiros do pior momento medido, sobrariam cerca de 680 MiB livres.

As páginas lidas do disco ficaram em cerca de 15 mil por segundo durante toda a
preparação e caíram a quase zero quando ela terminou. Isso acompanha a leitura
das fotos e o início de um processo por foto, não falta de memória, mas o
contador não separa as duas causas.

## Orçamento fixo de memória

A pedido do autor, a admissão passou a seguir o padrão encontrado em
[programas consolidados](2026-10-14-admissao-por-memoria-em-programas-consolidados.md):
as próprias reservas contra um teto fixo, de ¼ da RAM total, e um freio
quando a RAM ou o commit livres ficam abaixo da margem mais a estimativa do
novo trabalho. Os trabalhos em execução deixam de contar duas vezes. A regra
está na [revisão da admissão por memória](../design/0032-revisao-da-admissao-por-memoria.md).

As três rodadas abaixo foram feitas em sequência, com abertura fria e um
Projeto por vez.

| Rodada | Esperas por memória | RAM livre mínima | Primeiro editor | Segundo editor | Tempo por foto |
|---|---:|---:|---:|---:|---:|
| 2 Projetos, regra anterior | 69 (29,7 s) | 996 MB | 100,3 s | 197,3 s | 445 ms |
| 2 Projetos, orçamento fixo | 0 | 814 MB | 68,6 s | 154,8 s | 359 ms |
| 1 Projeto, orçamento fixo | 0 | 1311 MB | 118,8 s | — | 500 ms |

A regra fez o que devia: nenhuma espera por memória, e a RAM livre não passou
de 814 MB para baixo. Os tempos, porém, não servem para comparar as regras. O
tempo mediano de cada foto subiu de uma rodada para a seguinte, de 359 para
500 ms, porque o computador hospedeiro estava em uso, com 68% de carga.

O que dá para comparar é o trabalho depois do primeiro editor. Com o orçamento
fixo, dois Processadores de Imagens rodaram juntos por mais tempo: 53,2 s,
contra 32,8 s na rodada anterior com a regra antiga. Mesmo assim, as cerca de
295 fotos restantes levaram praticamente o mesmo tempo nas duas: 83 s e 81 s.
O processador da máquina virtual ficou entre 95% e 99% durante toda a
preparação, nas duas regras.

Isso corrige a conclusão da seção anterior. As esperas por memória não eram o
que segurava o segundo Projeto. Com dois núcleos e um editor aberto, o
processador já está ocupado: quando o segundo Processador de Imagens esperava,
o editor e o WebView usavam o núcleo livre. A preparação é limitada pelo
processador.

O Processador de Imagens decodifica o JPEG inteiro, de 24 a 30 MP, e só depois
reduz para a prévia, de no máximo 1600 pixels no lado maior
(`CACHE_MAX_EDGE_PX`). A libvips reduz já na leitura
([`thumbnail.c`](https://github.com/libvips/libvips/blob/v8.18.7/libvips/resample/thumbnail.c#L1215)).
Essa redução não foi medida no MyAlbuns.

As rodadas anteriores com a janela nativa no perfil `cliente-estimado` não
servem para medir o tempo total da abertura. O roteiro do laboratório esperava o
diálogo WebView fechar. Como esse diálogo não existe mais, as fases de uso
começaram com o editor ainda oculto, e o aplicativo foi fechado antes de a
preparação terminar.

## Outros achados da medição

- **Preparação tudo ou nada.** As prévias geradas só entram no índice do Cache
  quando a preparação inteira termina (`publish_prepared_batch`, chamado uma vez
  por preparação). Quando o processo foi encerrado no meio da preparação, 215
  prévias geradas foram descartadas, e a abertura seguinte refez as 300.
- **Um processo por imagem.** O Processador de Imagens é iniciado uma vez por
  foto. O intervalo entre um processo terminar e o próximo começar foi de 47 ms
  na mediana, cerca de 13% do tempo de preparação.
- **Envio de texturas.** Na segunda abertura, o envio de texturas ao WebGL
  (`texImage2D`) somou 722 ms ao navegar entre Lâminas, com uma pausa de 468 ms.
- **Rolagem do Canvas.** Durante a rolagem, `CanvasHorizontalScrollbar` escreveu
  a posição a cada quadro, cerca de 88 ms no total. No mesmo intervalo, o Painel
  de imagens e o Inspetor renderizaram de novo sem mudança própria. O tempo de
  resposta à roda do mouse chegou a 77–142 ms no percentil 95.

Esses pontos ficam para investigações próprias.

## Limites

- A máquina virtual limita memória e taxa de disco, mas não imita o tempo de
  busca de um disco mecânico.
- O limite de 25% do processador não foi aplicado. O hipervisor do hospedeiro
  usa o agendador raiz, e uma tarefa de 20 s levou o mesmo tempo com limite de
  25% e de 100%. Os dois processadores virtuais trabalham como dois núcleos
  inteiros de um i5-13450HX, bem mais rápidos que os de um computador antigo.
  Por isso, os tempos de preparação medidos são menores que os do usuário. A
  comparação entre regras continua válida, porque os dois núcleos somam como
  núcleos reais. Também por isso, carga no hospedeiro durante uma rodada muda o
  resultado.
- A máquina virtual não tinha rede, então o efeito do WPAD não foi medido.
- A janela nativa ainda não tem nome acessível próprio. O título da janela
  continua sendo o nome do aplicativo.
- Num computador sem WebGL2, a página do editor pode verificar os gráficos
  antes da tela de boas-vindas. Nesse caso o aviso gráfico do editor pode
  aparecer por um instante, antes de o Host ser encerrado. Esse caso não foi
  reproduzido.
- Uma decisão de Recuperação ou de Cópia externa já na tela não é cancelada
  pela recusa da verificação gráfica.
