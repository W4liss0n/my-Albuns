---
status: accepted
document: design
date: 2026-09-22
---

# Visualizador de imagens em janela própria

O Painel de fotos e as fotos aplicadas à Lâmina abrem uma janela separada do Projeto. A janela mostra a imagem inteira, sem recorte, rotação ou efeitos do Quadro. Visualizar não altera a seleção criativa, o histórico, a revisão ou o estado de salvamento. O Projeto permanece atrás, temporariamente bloqueado enquanto a janela owned está aberta.

## Entrada e sequência

O menu de contexto oferece **Visualizar imagem** com o atalho **Espaço**. No Painel, o item clicado com o botão direito é o inicial mesmo quando várias fotos continuam selecionadas. Anterior e próxima percorrem apenas as fotos visíveis na ordenação, filtro e pasta atuais, congelados durante a sessão. Espaço usa a foto com foco ou a âncora da seleção. Decorativos não entram nessa sequência.

Na área de edição, a foto selecionada abre ao **soltar** Espaço. Pressionar um ponteiro enquanto Espaço está ativo cancela a abertura; Espaço com arraste continua movendo a área de edição. A navegação percorre as fotos da Lâmina da foto inicial na ordem da composição, eliminando usos repetidos da mesma mídia. Campos de texto, controles e menus conservam suas teclas.

## Janela e interação

A barra própria da janela contém somente o nome da imagem, truncado se necessário, e os controles da janela. Não há barra de ferramentas, contador nem porcentagem de zoom. As setas de anterior e próxima ficam sobrepostas, centralizadas nas laterais da imagem, como ícones sem discos de fundo. Um botão discreto de **Ajustar à janela** aparece quando há ampliação. Setas do teclado navegam; roda amplia ao redor do ponteiro; `+` e `-` ampliam e reduzem; `0` ajusta; arrastar move a imagem ampliada. O zoom é limitado entre o encaixe e 800% e retorna ao encaixe ao trocar de imagem ou redimensionar a janela. A imagem permanece proporcional e inteira no encaixe. Esc ou fechar restaura o Projeto e o foco da origem disponível.

Carregamento, ausência e erro usam a área reservada para a imagem. Uma prévia anterior retida durante indisponibilidade recebe aviso visível. A janela do Projeto é a única dona da sequência e da demanda de prévias: imagem atual e duas vizinhas. A janela filha recebe só a apresentação atual e envia pedidos de navegação correlacionados com a sessão; ela não lê arquivos nem prepara cache. O Host só publica URLs opacas já autorizadas e libera o protocolo de cache para a janela filha durante a sessão ativa, revogando no fechamento, falha ou recuperação da interface do Projeto. Os comandos do Projeto ficam bloqueados desde o pedido de abertura, antes de a janela filha concluir a prontidão.

A política vigente do Cache limita a prévia a **1.600 pixels no maior lado**. Ampliar a janela aumenta a exibição dessa prévia; não promete pixels do Original. A origem e as dimensões do arquivo continuam administradas pelo Host e pelo Core.

## Aceitação

Verificar entradas do Painel e da Lâmina, ordenação e filtro, mídia clicada fora da âncora, deduplicação, Espaço com arraste, bloqueio das ações do Projeto, navegação nas extremidades, zoom/ajuste, restauração de foco, carregamento tardio, ausência, nome longo e janela estreita. A prova nativa deve cobrir abertura, URL do cache na janela filha, navegação, Esc/fechar, reabilitação do owner e reabertura, inclusive falha de abertura.
