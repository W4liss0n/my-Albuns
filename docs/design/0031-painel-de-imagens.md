---
status: accepted
document: design
date: 2026-09-10
ticket: 24
platform: windows
implementation-readiness: ready-for-agent
---

# Painel de imagens

Este contrato complementa a especificação do Painel e os contratos de
[estado](0012-propriedade-de-estado-e-modulos-do-nucleo.md) e
[importação](0020-importacao-com-decode-unico-e-lotes.md).

## Catálogo, preferências e ausência

O Painel mantém a seleção e a âncora atuais e uma busca independente por aba
durante a Janela aberta. A seleção se restringe aos itens da aba visível.
A seleção solicitada após importar é um evento consumido uma vez; alterações
de ordenação e filtro não o repetem. Ocultar o Painel conserva esse estado;
trocar de Projeto o reinicia. Aba, chave e direção da ordenação, filtro de uso
e tamanho das miniaturas pertencem ao armazenamento global de preferências.

As miniaturas de Fotos e Decorativos não exibem tag nem contador de usos.
A indicação visual de item usado acontece pelo esmaecimento da miniatura.

O slider usa um único tamanho compartilhado entre Fotos e Decorativos. Ajustar
ou restaurar o tamanho em qualquer aba vale para as duas; alterná-las conserva
o valor. A preferência permanece global à máquina e não integra o Projeto.
O arquivo de preferências da interface passa à versão 2 com um único campo
de tamanho. Ao ler a versão 1, adota o tamanho salvo de Fotos e conserva as
demais preferências; a leitura não regrava o arquivo. A próxima alteração de
preferência publica a versão 2 sem os antigos tamanhos por aba.

O Host fornece criação, alteração e disponibilidade a partir das observações
estabilizadas do Monitor, correlacionadas com o vínculo atual. Essa consulta
não acessa Originais nem depende da demanda de miniaturas. Ausentes ficam
depois dos demais arquivos em qualquer direção de ordenação; datas conhecidas
precedem desconhecidas e o Nome natural desempata.

O aviso de ausentes abre uma revisão temporária. Durante essa revisão, busca,
filtros de uso e aba ficam sobrepostos às escolhas anteriores. Encerrá-la
retoma as escolhas preservadas, sem gravar a revisão como preferência.

## Importação e gestos

Fotos e Decorativos usam a mesma tentativa nativa. A aba é capturada ao iniciar
a ação e define o tipo de todos os novos vínculos. O Core distingue duplicatas
por tipo e caminho: o mesmo Original pode ter um vínculo em cada aba.
Arquivos, pasta e soltura convergem antes do processamento e do único commit.
Uma pasta fornece imagens diretamente contidas nela, sem percorrer subpastas.
Arquivos explicitamente selecionados passam pela inspeção mesmo quando a
extensão não corresponde ao conteúdo; o formato real decide a aceitação.

JPEG, PNG e TIFF atravessam o processamento já usado pelas Fotos. O raster
decodificado fornece dimensões e prévia, conservando transparência. Inspeção
alternativa, falhas parciais, adoção no Monitor e publicação do Cache mantêm
seus donos. A associação de uma prévia ao vínculo definitivo usa tipo e caminho,
para não confundir Foto e Decorativo do mesmo Original.

A soltura do Windows é recebida pelo WebView e convertida de coordenadas
físicas para pixels CSS antes do teste de pertencimento à área do Painel.
O gesto interno usa eventos de ponteiro e o limiar de arraste do Windows.
Assim, mover uma miniatura dentro do programa coexiste com a recepção de
arquivos externos. Solturas de Fotos consultam o alvo atual no Core;
respostas de movimentos anteriores e gestos cancelados não fazem commit.

Duplo clique e soltura de uma Foto ou Decorativo já importado atualizam a
Lâmina imediatamente, sem abrir `Processando Imagens` nem aguardar uma nova
preparação do Cache. A composição reutiliza a prévia disponível; a recuperação
de prévias ausentes acontece em segundo plano. Desfazer e refazer a aplicação
seguem a mesma regra.

O Host captura raízes do catálogo, Cache e seleção antes de enumerar pastas.
A enumeração e o processamento usam o mesmo plano congelado; filhos diretos
conservam o caminho lógico da pasta selecionada.

No Windows, a soltura chega ao Host como `WindowEvent::DragDrop`. O Host conserva
os `PathBuf` nativos de somente uma soltura ainda não consumida e envia ao WebView
um identificador opaco com a posição física. Uma importação aceita consome essa
soltura uma única vez; a próxima substitui a pendência anterior. Isso evita a
conversão de caminhos para texto Unicode: o serializador padrão de `PathBuf`
usado pelo evento de arraste do Tauri rejeita caminhos que não sejam UTF-8.
`Leave` conserva a pendência, pois pode chegar antes do comando assíncrono que
consome uma soltura já aceita.

## Remoção da seleção

O Painel é dono do alvo de `Delete` e do menu `Remover`. Campos de texto
conservam seu tratamento de teclado; a seleção e a âncora continuam transitórias.
A confirmação aguarda comandos já pendentes antes de contar os usos e bloqueia
novas interações enquanto estiver aberta. A decisão referencia uma única revisão
criativa; outra revisão exige revisar a seleção novamente.

O Core recebe a seleção inteira em uma ação. Remover tudo elimina Frames
destravados que usavam as Fotos; posições travadas são esvaziadas. Manter Frames
esvazia todas as ocorrências e preserva a estrutura. Itens sem uso saem junto
com o restante da seleção. Os Originais permanecem intactos e Undo restaura o
catálogo e a composição juntos.

## Aplicações de Decorativos

Cada Lâmina guarda personalizações independentes de Fundo e Overlay. Cada papel
pode acompanhar o padrão inteiro, conter uma aplicação personalizada de Ambos
os lados ou possuir decisões independentes à esquerda e à direita. Uma decisão
por lado conserva a origem herdada ou o conteúdo personalizado e sua área de
mapeamento: Página ou Lâmina inteira.

Dividir uma aplicação de Ambos os lados conserva no lado oposto o mapeamento
da imagem inteira, limitado por um recorte da Página. O Core entrega área de
desenho e recorte separadamente ao Canvas e à exportação. Um lado herdado resolve
sempre o padrão atual; portanto, uma mudança posterior de conteúdo ou escopo
acompanha o padrão sem substituir a personalização do outro lado.

Aplicações de Ambos os lados usam a superfície ativa: em Página única ocupam
a Página e se expandem quando a Lâmina volta a ser dupla. Aplicações específicas
de um lado permanecem específicas desse lado. Converter para Página única
descarta as personalizações do lado desativado; voltar para dupla inicia esse
lado no padrão. A decisão e o recorte do lado que permanece ativo são conservados.
O papel de Fundo nunca altera
o Overlay, a geometria dos Frames ou a seleção transitória.

O formato público passa à versão 11. Ele conserva o conteúdo da versão 10 e
acrescenta `sheetVisuals`, uma lista de personalizações identificadas por Lâmina.
Lâminas omitidas acompanham integralmente o padrão. O carregamento rejeita
identificadores desconhecidos ou duplicados e referências que não sejam
Decorativos existentes. Versões anteriores começam sem personalizações locais.

O protocolo do Processador passa à versão 22 para impedir que um binário antigo
ignore os recortes enviados pelo Host. O snapshot de renderização conserva a
versão 6: o campo opcional de recorte mantém a leitura dos snapshots anteriores;
a negociação do protocolo exige o consumidor que sabe aplicá-lo.

A faixa central ocupa 20% da largura da Lâmina dupla, entre 40% e 60%. O Core
devolve a zona atingida e sua composição temporária. A interface reutiliza essa
prévia enquanto o ponteiro permanece na mesma zona e papel; a soltura sempre
consulta o ponto final e o comando confirma o alvo novamente na sessão atual.
Respostas atrasadas, cancelamento e soltura fora da superfície não fazem commit.

O feedback visual do arraste contorna a área que efetivamente receberá a imagem:
a Página inteira à esquerda ou à direita, ou ambas as Páginas. A zona de
ponteiro de 40%/20%/40% serve à escolha do escopo e não define esse contorno.
Um traço azul de 2 pixels, com apoio branco para contraste, acompanha a área
ativa visível e conserva sua espessura no Zoom. Nenhuma tonalidade é aplicada
sobre a composição. Duas guias verticais tracejadas de 1 pixel delimitam a
região central. Cada guia tem um único traço azul, sem halo ou traço paralelo.
Os traços e intervalos têm 5 pixels e começam a 8 pixels das bordas superior e
inferior, conservando essas medidas no Zoom. As guias ficam mais visíveis ao
atingir Ambos os lados, sem preencher a faixa nem desenhar colchetes;
não aparecem em Página única.

Durante o gesto, o foco da Lâmina, os indicadores de seleção dos Frames e a
Barra ficam visualmente ocultos para deixar claro o destino do Decorativo.
Suas seleções permanecem intactas e voltam a ser mostradas ao encerrar o
arraste. O rótulo junto ao ponteiro conserva o papel e o lado de aplicação,
inclusive na troca entre Fundo e Overlay com `Shift`.

Os indicadores de uso separam Frames, Fundos, Overlays, padrão de Fundo e padrão
de Overlay. Um padrão continua contando como uso mesmo quando personalizações
ocultam todas as suas aplicações. Remover um Decorativo restaura o padrão nos
alvos personalizados que o referenciavam; remover o próprio padrão usa branco
para Fundo e ausência para Overlay, preservando as outras personalizações.

## Contratos externos

Versões verificadas: React 19.2.8, API JavaScript do Tauri 2.11.1,
Tauri Rust 2.11.5, diálogo 2.7.2 e `image` 0.25.10. `find-docs` foi usado
antes das decisões; a consulta indexada atingiu a cota. A confirmação usou
documentação oficial e as interfaces da versão instalada.

A [configuração do Tauri](https://v2.tauri.app/reference/config/#dragdropenabled)
documenta a substituição do arraste HTML5 pelo manipulador nativo no Windows.
[DragDropEvent](https://docs.rs/tauri/2.11.5/tauri/enum.DragDropEvent.html)
define caminhos nativos e posições físicas. O evento opaco próprio chega pela
assinatura da Janela, cancelada ao desmontar o adaptador. O código
instalado de `tauri-plugin-dialog` confirma `pick_files` e `pick_folder`
assíncronos e o resultado opcional de cancelamento.
O processamento reutiliza os decodificadores de PNG e TIFF já presentes no
programa e as APIs verificadas no contrato de importação.

O Canvas usa PixiJS 8.19.0. A documentação da versão instalada
(`effectsMixin.d.ts`, propriedade `mask`) exige que a máscara pertença à árvore
do pai do objeto; cada recorte é um `Graphics` irmão do raster, com o mesmo ciclo
de vida da Lâmina. A consulta `find-docs` para máscaras também atingiu a cota;
a interface instalada confirmou o contrato documentado de máscaras do PixiJS 8.

A captura dos gestos usa o protocolo W3C WebDriver implementado pelo Edge e
seu driver pareado, cujas versões ficam registradas na evidência. O contrato de
[ações do WebDriver](https://www.w3.org/TR/webdriver2/#actions) define `Shift`
como U+E008 e conserva teclas pressionadas entre ações; a captura libera as
fontes de entrada depois da imagem. A consulta indexada também atingiu a cota
neste caso; a especificação oficial confirmou esse comportamento.

Para a revisão do feedback, `find-docs` confirmou a API de
[linhas de Graphics do PixiJS 8](https://pixijs.com/8.x/guides/components/scene-objects/graphics/graphics-pixel-line).
`pixelLine` mantém um único pixel independentemente da escala; por isso o
contorno de 2 pixels usa largura compensada pela escala do Canvas. Os tipos
instalados de Graphics e StrokeStyle da versão 8.19.0 confirmam `clear`,
`alignment` interno e `pixelLine` usado nas guias centrais.

A migração de preferências usa Serde 1.0.229 e serde_json 1.0.151. A consulta
`find-docs` confirmou os atributos de [valor padrão](https://serde.rs/attr-default.html)
e [omissão na serialização](https://serde.rs/attr-skip-serializing.html): os
campos antigos são aceitos somente na leitura e não voltam ao arquivo salvo.
