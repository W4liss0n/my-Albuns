---
status: current
document: research
date: 2026-10-07
platform: windows-11-x64
---

# Vários Projetos abertos

Dois problemas relatados em 7/10/2026 foram medidos no aplicativo e corrigidos:
abrir vários Projetos pelo Explorador abria um de cada vez, e com vários
Projetos abertos o editor inteiro ficava lento, no Painel de imagens e no
Canvas. A base de código é o commit `3f7cb6fd`.

## Como foi medido

- Compilação de desenvolvimento (`--debug`), com o Processador de Imagens
  compilado no perfil de desenvolvimento, que já otimiza o código de imagem.
- Dados isolados em `MYALBUNS_PROCESS_GATE_DATA_ROOT`, fora dos dados do
  aplicativo instalado.
- Seis Projetos de teste locais, cada um com 40 fotos JPEG de 6000 × 4000
  pixels, quatro por Lâmina, em dez Lâminas.
- Os seis arquivos são abertos como o Explorador abre: um processo por
  arquivo, 80 ms entre eles.
- A fluidez é medida no editor da frente, pelo DevTools do WebView2:
  intervalo entre quadros e tarefas longas durante 40 giros da roda sobre o
  Canvas, 20 giros sobre o Painel e 2 s parado. A tela é de 120 Hz, então o
  quadro ideal tem 8,3 ms.
- Computador: Intel i5-13450HX (16 threads), 24 GB de RAM, cerca de 3,5 GB
  livres antes de abrir o MyAlbuns.
- Os tempos de abertura são da compilação de desenvolvimento, não da versão
  instalada que o usuário usa; a causa da lentidão do Canvas não depende da
  compilação.

Os registros do usuário no mesmo dia confirmam o padrão em Projetos reais na
rede: dez ativações encaminhadas, cada uma levando de 24 a 61 s, uma depois da
outra.

## Abrir vários Projetos

### Causa

O Global atendia uma abertura por vez. Cada ativação vinda do Explorador
esperava a anterior terminar, e o Host só termina depois de preparar todas as
fotos sem Cache e mostrar a janela. Duas condições impediam simplesmente abrir
em paralelo:

- o Host que inicia tentava a concessão de manutenção do Cache sem esperar e a
  mantinha durante a verificação das prévias; um segundo Host iniciado ao
  mesmo tempo falhava;
- a janela de progresso tinha um único rótulo, e criar outra destruía a
  primeira.

### Resultado

| Seis Projetos | Antes | Depois |
| --- | ---: | ---: |
| Sem Cache: última janela | 109,5 s | 24,4–27,6 s (três aberturas) |
| Com Cache: última janela | 22,3 s | 8,7–14,8 s (sete aberturas, mediana 10,7 s) |

Antes, as janelas apareciam aos 13,5, 26,6, 46,6, 67,2, 89,1 e 109,5 s. Depois,
todas aparecem entre 20 e 28 s, porque todos os Projetos preparam suas fotos ao
mesmo tempo. Uma primeira medição sem Cache, que terminou aos 54,8 s, foi
descartada: o Processador da versão de lançamento ignora a pasta de dados
isolada e recusou todas as fotos, então nenhuma prévia foi preparada.

### O que mudou

- **Aberturas simultâneas.** Arquivos abertos pelo Windows compartilham a
  coordenação do Global; Boas-vindas, criação, recentes, Exportação em lote e
  pedidos de Configurações ou de novo Projeto continuam exclusivos.
- **Uma janela de progresso.** Os Projetos abertos juntos aparecem em uma só
  janela, cada um com sua contagem de fotos. Com um Projeto, a janela é a
  mesma de antes. Decisões de Recuperação e de Cópia externa aparecem uma de
  cada vez nessa janela, com o nome do Projeto.
- **Foco.** Dos arquivos abertos juntos, somente o primeiro editor pronto
  recebe o foco. O pedido de foco do editor contorna a trava de primeiro plano
  do Windows simulando a tecla Alt, então o próprio Windows não impedia que
  cada editor pronto viesse para a frente; mostrar uma janela que reabre
  maximizada também a ativa. Agora o Global cria um sinal com nome para cada
  grupo de arquivos que chegam juntos, e só o primeiro Host a consumi-lo pede
  o foco. Os outros aparecem encobertos, vão para trás da janela que tinha o
  foco e só então ficam visíveis, piscando na barra de tarefas. Nos registros
  de uma abertura de seis Projetos maximizados, um único Host pediu o foco e
  nenhum outro foi ativado; o primeiro plano ficou no primeiro editor até a
  pessoa trocar de janela.
- **Host preparado para iniciar junto.** A concessão de manutenção do Cache é
  aguardada por até 30 s e devolvida logo depois da reserva do namespace; a
  verificação das prévias continua protegida pela reserva do namespace.
- **Processadores de todos os Projetos somados.** Os Processadores de Imagens
  de todos os processos ocupam no máximo uma vaga por núcleo lógico menos um
  (até oito). Com pouca memória livre, um só trabalha de cada vez no
  computador inteiro, e não um por Projeto. Os Processadores de um Projeto
  fora de foco rodam com prioridade abaixo do normal.
- **Prazo de abertura.** O Global espera o Host por até 300 s sem notícias;
  cada progresso recebido reinicia esse prazo, com teto de 2 horas.

## Fluidez com vários Projetos abertos

### Causa

O Canvas de cada Janela do Projeto redesenhava a cena inteira a cada quadro da
tela, mesmo sem nenhuma mudança. Um Projeto parado custava cerca de 31% de um
núcleo no processo de GPU do WebView2 e 21% no renderizador. Janelas
maximizadas atrás do editor da frente continuaram gastando de 13% a 18% de um
núcleo cada: o WebView2 não as pausava.

Com seis Projetos abertos e parados, o editor da frente disputava a GPU com os
outros cinco. Minimizar os outros cinco devolvia exatamente os números de um
Projeto sozinho, o que isolou a causa. A memória não muda ao minimizar e não
explicava a lentidão.

### Resultado

Editor da frente com cinco outros Projetos abertos e parados:

| Medição | 1 Projeto | 6 Projetos, antes | 6 Projetos, depois |
| --- | ---: | ---: | ---: |
| Canvas, roda: quadro mediano | 8,3 ms | 33,4–41,7 ms | 8,3 ms |
| Canvas, roda: percentil 95 | 8,5–16,7 ms | 75–108 ms | 8,4–16,5 ms |
| Painel, rolagem: percentil 95 | 8,5 ms | 75–108 ms | 8,4–16,5 ms |
| Parado: percentil 95 | 8,5 ms | 75–117 ms | 8,5 ms |
| GPU do WebView2, seis Projetos parados | — | 77% de um núcleo (janelas em cascata) | 0–0,2% |
| CPU de cada Projeto parado | 53,7% | 15–22% | 0–2,5% |

"Antes" reúne as janelas em cascata e as seis maximizadas; o pior caso era o
das maximizadas.

### O que mudou

O Canvas desenha somente quando algo pode mudar na tela: atualização do
Projeto, textura carregada, redimensionamento, entrada do usuário sobre o
Canvas, animação de reordenação e transição da barra da Lâmina. Meio segundo
depois da última mudança, os relógios do PixiJS param. A verificação no
aplicativo, depois de rolar o Canvas, passar o mouse sobre uma Lâmina e rolar o
Painel, não registrou nenhum pedido de quadro em 2 s parados, e a imagem parada
foi idêntica à imagem redesenhada em seguida.

## O que continua

- **Memória.** Cada Projeto aberto mantém cerca de 900 MB no WebView2, sendo
  cerca de 550 MB privados; o processo de GPU guarda cerca de 200 MB do
  Canvas. Nos registros do uso real de 7/10/2026, com dez Projetos na rede, a
  memória livre lida pela admissão do Processador chegou a 595 MB.
  Isso não causava a lentidão medida, mas pode causar paginação em máquinas
  com menos memória.
- **Rede.** Cada demanda de prévias observa o catálogo inteiro antes de
  responder, e cada Host lista as pastas a cada segundo e abre todos os
  Originais a cada 30 s. Em Projetos na rede, isso soma o tráfego de todos os
  Projetos abertos e não foi medido aqui.
- **Mesmo Projeto por dois caminhos.** Se uma abertura conjunta tiver o mesmo
  Projeto por uma unidade mapeada e pelo caminho de rede, a segunda abertura
  espera as outras terminarem e então focaliza a janela já aberta.
