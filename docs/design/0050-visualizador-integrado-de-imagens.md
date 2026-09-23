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

A barra própria da janela contém somente o nome da imagem, truncado se necessário, e os controles da janela. Não há barra de ferramentas, contador nem porcentagem de zoom. As setas de anterior e próxima ficam sobrepostas, centralizadas nas laterais da imagem, como ícones sem discos de fundo. Um botão discreto de **Ajustar à janela** aparece quando há ampliação. Setas do teclado navegam; roda amplia ao redor do ponteiro; `+` e `-` ampliam e reduzem; `0` ajusta; arrastar move a imagem ampliada. O zoom é limitado entre o encaixe e 800% e retorna ao encaixe ao trocar de imagem ou redimensionar a janela. A imagem permanece proporcional, centralizada e inteira no encaixe, com pelo menos 24 px de superfície neutra entre ela e cada borda da área de visualização. O espaçamento também vale para retratos e janelas estreitas; a ampliação pode ocupar essa margem. Esc ou fechar restaura o Projeto e o foco da origem disponível.

Carregamento, ausência e erro usam a área reservada para a imagem. Uma prévia anterior retida durante indisponibilidade recebe aviso visível. A janela do Projeto é a única dona da sequência e da demanda de prévias: imagem atual e duas vizinhas. A janela filha recebe só a apresentação atual e envia pedidos de navegação correlacionados com a sessão; ela não lê arquivos nem prepara cache. O Host só publica URLs opacas já autorizadas e libera o protocolo de cache para a janela filha durante a sessão ativa, revogando no fechamento, falha ou recuperação da interface do Projeto. Os comandos do Projeto ficam bloqueados desde o pedido de abertura, antes de a janela filha concluir a prontidão.

A política vigente do Cache limita a prévia a **1.600 pixels no maior lado**. Ampliar a janela aumenta a exibição dessa prévia; não promete pixels do Original. A origem e as dimensões do arquivo continuam administradas pelo Host e pelo Core.

## Correção de olhos no visualizador

A comparação ocupa duas metades com áreas de imagem de mesma altura útil: referência à esquerda e imagem a corrigir à direita. Nesse modo, os nomes dos arquivos ficam ocultos tanto nos painéis quanto na barra da janela; os controles da janela permanecem disponíveis. Os rótulos curtos **Referência**, **Imagem a corrigir**, **Original** e **Corrigida** continuam orientando a comparação, e as imagens mantêm seus nomes acessíveis.

As ações são uma extensão da ferramenta **Abrir olhos**, no canto superior direito da área de visualização. Ao entrar na correção, o mesmo ponto passa a oferecer **Fechar correção**, conservando posição, tamanho e tratamento visual. Os outros ícones se estendem para a esquerda, próximos entre si; uma revelação curta e discreta parte do acionador para reforçar a continuidade. A preferência do sistema por movimento reduzido elimina essa animação. O grupo flutua sobre a área do visualizador, sem barra de fundo, caixa ou rodapé. Os rótulos das fotos e as ferramentas mantêm espaço entre si, inclusive na janela estreita. Ao sair da correção, a janela volta a mostrar o nome da foto.

O grupo oferece **Usar esta foto** ao escolher a referência, **Trocar referência** após essa escolha e **Ver correção** quando os dois rostos estiverem selecionados. Com o resultado pronto, oferece **Antes e depois** e **Salvar correção**. Antes e depois alterna somente a foto de destino entre original e resultado, sem alterar a referência, o enquadramento, o Projeto ou a correção preparada. A alternância tem estado acessível e indicação discreta; trocar a referência ou preparar outro resultado encerra a comparação anterior. Salvar confirma a correção preparada mesmo se a pessoa estiver olhando o original na comparação. Essa ação mantém o fluxo existente de criar uma cópia corrigida e atualizar a foto usada no Projeto; não sobrescreve o Original nem substitui o salvamento do arquivo do Projeto.

Erros e progresso essenciais aparecem fora do fluxo, próximos às ferramentas, sem deslocar as imagens. O erro completo continua acessível pelo tooltip compartilhado. A prévia dispensa instruções repetidas; não há ações nem faixa de mensagens no rodapé.

As setas, **Ajustar à janela**, **Abrir olhos** e suas ações de correção compartilham a ferramenta de imagem: alvo de 42 px, setas de 26 px e ferramentas de 18 px, com glifos separados visualmente da fotografia, sem disco de fundo, tooltip React Aria no hover ou foco, nome acessível e foco visível. Não há tooltip nativo em paralelo. Os marcadores de rosto têm alvo confortável e indicação pequena, deslocada do centro do rosto, limitada à área visível, com seleção anunciada por `aria-pressed`. Salvar, fechar a correção e trocar de referência permanecem bloqueados enquanto a correção é salva.

## Aceitação

Verificar entradas do Painel e da Lâmina, ordenação e filtro, mídia clicada fora da âncora, deduplicação, Espaço com arraste, bloqueio das ações do Projeto, navegação nas extremidades, zoom/ajuste, restauração de foco, carregamento tardio, ausência, nome longo e janela estreita. A prova nativa deve cobrir abertura, URL do cache na janela filha, navegação, Esc/fechar, reabilitação do owner e reabertura, inclusive falha de abertura.
