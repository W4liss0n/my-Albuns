---
status: accepted
document: design
date: 2026-09-22
updated: 2026-09-23
---

# Visualizador de imagens em janela própria

O Painel de fotos e as fotos aplicadas à Lâmina abrem uma janela separada do Projeto. A janela mostra a imagem inteira, sem recorte, rotação ou efeitos do Quadro. Visualizar não altera a seleção criativa, o histórico, a revisão ou o estado de salvamento. O Projeto permanece atrás, temporariamente bloqueado enquanto a janela owned está aberta.

## Entrada e sequência

O menu de contexto oferece **Visualizar imagem** com o atalho **Espaço**. No Painel, o item clicado com o botão direito é o inicial mesmo quando várias fotos continuam selecionadas. Anterior e próxima percorrem apenas as fotos visíveis na ordenação, filtro e pasta atuais, congelados durante a sessão. Espaço usa a foto com foco ou a âncora da seleção. Decorativos não entram nessa sequência.

Na área de edição, a foto selecionada abre ao **soltar** Espaço. Pressionar um ponteiro enquanto Espaço está ativo cancela a abertura; Espaço com arraste continua movendo a área de edição. A navegação percorre as fotos da Lâmina da foto inicial na ordem da composição, eliminando usos repetidos da mesma mídia. Campos de texto, controles e menus conservam suas teclas.

## Janela e interação

A barra própria da janela contém somente o nome da imagem, truncado se necessário, e os controles da janela. Não há barra de ferramentas, contador nem porcentagem de zoom. As setas de anterior e próxima ficam sobrepostas, centralizadas nas laterais da imagem, como ícones em grafite sobre um fundo circular claro e translúcido, sem sombra. Um botão discreto de **Ajustar à janela** aparece quando há ampliação. Setas do teclado navegam; roda amplia ao redor do ponteiro; `+` e `-` ampliam e reduzem; `0` ajusta; arrastar move a imagem ampliada. O zoom é limitado entre o encaixe e 800% e retorna ao encaixe ao trocar de imagem ou redimensionar a janela. A imagem permanece proporcional, centralizada e inteira no encaixe, com pelo menos 24 px de superfície neutra entre ela e cada borda da área de visualização. O espaçamento também vale para retratos e janelas estreitas; a ampliação pode ocupar essa margem. Esc ou fechar restaura o Projeto e o foco da origem disponível.

Carregamento, ausência e erro usam a área reservada para a imagem. Uma prévia anterior retida durante indisponibilidade recebe aviso visível. A janela do Projeto é a única dona da sequência e da demanda de prévias: imagem atual e duas vizinhas. A janela filha recebe só a apresentação atual e envia pedidos de navegação correlacionados com a sessão; ela não lê arquivos nem prepara cache. O Host só publica URLs opacas já autorizadas e libera o protocolo de cache para a janela filha durante a sessão ativa, revogando no fechamento, falha ou recuperação da interface do Projeto. Os comandos do Projeto ficam bloqueados desde o pedido de abertura, antes de a janela filha concluir a prontidão.

A política vigente do Cache limita a prévia a **1.600 pixels no maior lado**. Ampliar a janela aumenta a exibição dessa prévia; não promete pixels do Original. A origem e as dimensões do arquivo continuam administradas pelo Host e pelo Core.

## Correção de olhos no visualizador

A comparação ocupa duas metades com áreas de imagem de mesma altura útil: referência à esquerda e imagem a corrigir à direita. Nesse modo, os nomes dos arquivos ficam ocultos tanto nos painéis quanto na barra da janela; os controles da janela permanecem disponíveis. Não há títulos nem rótulos visuais sobre as metades, inclusive **Referência**, **Imagem a corrigir**, **Original** e **Corrigida**. As fotos ficam inteiras, proporcionais e centralizadas, com 24 px de respiro, sem reservar uma faixa para os antigos textos ou ferramentas. As imagens e as regiões mantêm identificação acessível.

As ações de correção são uma extensão da ferramenta **Abrir olhos**, no canto superior direito da área de visualização. Ao entrar na correção, o mesmo ponto passa a oferecer **Fechar correção**, conservando posição, tamanho e tratamento visual. O fundo circular do olho se prolonga para a esquerda em uma única cápsula translúcida, envolvendo **Antes e depois** e **Salvar correção** quando há um resultado, sem discos separados, divisórias, bordas ou sombra. Uma revelação curta e discreta parte do acionador para reforçar a continuidade. A preferência do sistema por movimento reduzido elimina essa animação. A cápsula ocupa apenas a largura dos comandos e flutua sobre as imagens; não cria barra de largura total nem rodapé. Ao sair da correção, a janela volta a mostrar o nome e a imagem atual, com encaixe, ampliação e navegação funcionando normalmente.

**Usar esta foto** ao escolher a referência e **Trocar referência** após essa escolha ocupam o mesmo ponto flutuante no centro inferior da metade esquerda, a 16 px da borda, com o círculo translúcido de ferramenta de imagem e sem rodapé. Selecionar um rosto no destino e um na referência prepara a correção automaticamente, sem botão **Ver correção**. Cada novo par completo dispara uma preparação; uma falha não provoca tentativas infinitas. Resultados de uma seleção anterior não podem substituir a seleção atual, reaparecer após cancelar ou ser salvos por engano. Com o resultado pronto, a cápsula oferece **Antes e depois** e **Salvar correção**. Antes e depois alterna somente a foto de destino entre original e resultado, sem alterar a referência, o enquadramento, o Projeto ou a correção preparada. A alternância tem estado acessível, tooltip e indicação neutra no próprio botão, sem texto sobre as fotos; trocar a referência ou preparar outro resultado encerra a comparação anterior.

**Salvar correção** abre o diálogo compartilhado **Substituir foto original?**, identificando o nome do arquivo sem sinais decorativos como « » e explicando que o original será substituído pela versão corrigida. As ações são **Cancelar** e **Substituir original**, com foco inicial seguro em Cancelar. Cancelar preserva a prévia para conferência. A substituição exige confirmação explícita mesmo se a pessoa estiver olhando o original em Antes e depois. Até essa confirmação, analisar, selecionar, comparar e fechar não alteram o arquivo original.

Após confirmar, a correção substitui o conteúdo do arquivo original, preservando seu caminho e formato. Essa decisão substitui a política anterior de manter uma cópia corrigida como novo vínculo. A gravação deve evitar deixar um arquivo incompleto em caso de falha e não pode sobrescrever silenciosamente uma foto alterada externamente desde a preparação. As prévias e os vínculos do Projeto precisam refletir o novo conteúdo; reabrir ou exportar deve usar a foto corrigida. O salvamento da correção não substitui o salvamento do arquivo do Projeto. Enquanto o diálogo está aberto, teclado e ponteiro pertencem à confirmação; Esc cancela somente o diálogo.

No modo de correção, não há mensagens soltas sobre as imagens. Ausência de rosto e falha de análise aparecem somente no tooltip compartilhado da própria foto afetada; erros ao preparar ou salvar pertencem à foto a corrigir. O aviso abre ao passar o ponteiro sobre a imagem ou focá-la pelo teclado e fecha ao sair dela. A âncora acompanha os limites visíveis da foto, sem incluir as margens vazias do painel. Sem imagem disponível, a área reservada mantém o acesso à explicação. Não há ícone de aviso separado, e os botões exibem somente as dicas de suas ações. Erros recuperáveis preservam a possibilidade de tentar novamente. Textos de andamento como **Analisando rostos**, **Preparando correção** e **Aplicando correção** são dispensados; os controles bloqueados e o estado acessível de ocupação comunicam o processamento. A prévia dispensa instruções repetidas; não há ações nem faixa de mensagens no rodapé.

As setas, **Ajustar à janela**, **Abrir olhos** e suas ações de correção compartilham a ferramenta de imagem: alvo de 42 px, setas de 26 px e ferramentas de 18 px, com ícones em grafite e superfície clara translúcida, sem sombra. Controles isolados usam círculos de 36 px; na correção, os comandos do olho compartilham a cápsula contínua de mesma altura e tom. A superfície usa o tema a 56% em repouso, com realce discreto sob o comando em hover, sem acumular camadas de opacidade. O tooltip React Aria no hover ou foco, o nome acessível e o foco visível permanecem disponíveis. Não há tooltip nativo em paralelo. Salvar, fechar a correção e trocar de referência permanecem bloqueados enquanto a correção é salva.

Cada rosto selecionável recebe uma caixa de contorno calculada a partir dos seus pontos detectados, sem número visível e sem preencher a área do rosto. Duas linhas finas, uma escura e uma clara, separam a caixa de retratos claros e escuros sem sombra difusa nem cor saturada. Ao selecionar, o contorno se firma e um pequeno **Check** em disco neutro contrastante integra o canto da caixa; `aria-pressed` anuncia o mesmo estado. A caixa acompanha o enquadramento da foto durante encaixe, ampliação, deslocamento e redimensionamento, sendo recortada pela área de visualização; não se desloca artificialmente para a borda quando o rosto sai de vista. O nome acessível identifica a foto e o rosto.

As caixas e a seleção continuam disponíveis durante a preparação, na prévia e
em Antes e depois. Selecionar outro rosto no destino ou na referência prepara
automaticamente o novo par, sem sair do modo de correção nem repetir a análise
da mesma foto. Os pontos continuam ligados à imagem original; a prévia corrigida
não provoca nova detecção. Cada par usa o destino original como base, sem
acumular o ajuste da seleção anterior. Um resultado antigo não pode aparecer
ou ser salvo para o novo par. Durante o salvamento, a seleção fica bloqueada;
a confirmação mantém sua exclusividade de teclado e ponteiro.

Com um rosto selecionado, roda e teclas de zoom aproximam esse rosto,
centralizando-o na medida permitida pelos limites da imagem. Selecionar por
si só não amplia a foto. Ao trocar de rosto com a imagem ampliada, o nível de
zoom é mantido e o foco acompanha o novo rosto. Cada metade conserva seu
enquadramento ao alternar a comparação ou receber a prévia. Ajustar à janela
e a tecla 0 continuam mostrando a foto inteira.

No Painel de imagens, pressionar Espaço não acrescenta um contorno externo à
miniatura. O foco de teclado usa a borda já existente do cartão compartilhado,
com tom neutro distinguível da seleção, sem mudar dimensões. Tab continua
indicando qual cartão recebe o teclado; fechar o visualizador restaura o foco.

### Continuidade ao navegar e trocar de modo

Ao avançar ou voltar, por botão ou teclado, a foto exibida permanece até que a
próxima esteja carregada e tenha seu tamanho calculado. Isso também vale para
a navegação da referência na correção de olhos. A substituição acontece de uma
vez, sem quadro vazio, sobreposição das duas fotos ou texto de carregamento
entre imagens. A foto de destino não muda ao navegar pela referência.

Enquanto a próxima foto carrega, a anterior serve apenas como continuidade
visual: não recebe zoom, seleção de rosto nem ações de correção em nome da nova
foto. O nome mostrado deve corresponder à foto visível. A navegação continua
disponível, e somente o pedido mais recente pode assumir a tela. Uma falha
definitiva ou uma nova sessão não mantém a foto de outro item como resultado.
Não há acúmulo de fotos retidas; basta a foto exibida e a próxima em preparação.

Os controles também permanecem estáveis durante a navegação. **Abrir olhos**
mantém posição, foco e aparência enquanto a próxima foto carrega; a ação fica
bloqueada e anuncia ocupação sem sumir, piscar, ganhar spinner ou texto novo.
O mesmo vale para **Usar esta foto** na referência e para **Ajustar à janela**
enquanto uma foto ampliada permanece retida. Ao mostrar a próxima foto já
encaixada, o ajuste deixa de ser necessário. As setas pertencem à área de
visualização e não são recriadas junto com cada foto. Uma indisponibilidade
definitiva mantém a indicação visual de ação desabilitada; não há ação sobre
uma foto anterior em nome da atual.

Ao entrar na correção de olhos, a foto já exibida permanece visível até que a
imagem de destino esteja carregada e tenha seu tamanho calculado. A troca não
passa por uma área vazia nem usa uma animação de desaparecimento para disfarçar
o carregamento. A referência pode carregar independentemente; sua demora ou
falha não bloqueia o fechamento da correção. A detecção de rostos também não
atrasa a apresentação das fotos.

Ao sair, o visualizador reaproveita a foto já carregada, sem apagá-la e carregá-la
de novo. A retenção respeita a identidade da foto e da sessão: navegar ou salvar
uma nova versão não pode reapresentar conteúdo antigo como se fosse o atual.
Somente o modo ativo recebe foco e ações; a confirmação de salvamento continua
sendo exclusiva quando aberta.

## Aceitação

Verificar entradas do Painel e da Lâmina, ordenação e filtro, mídia clicada fora da âncora, deduplicação, Espaço com arraste, bloqueio das ações do Projeto, navegação nas extremidades, zoom/ajuste, restauração de foco, carregamento tardio, ausência, nome longo e janela estreita. A prova nativa deve cobrir abertura, URL do cache na janela filha, navegação, Esc/fechar, reabilitação do owner e reabertura, inclusive falha de abertura. Para correção, verificar preparação automática, cancelamento com processamento pendente, erro sem repetição automática, retorno à imagem e navegação ao fechar o modo, confirmação de substituição e cancelamento do diálogo sem alterar o arquivo. Em cópias de teste, conferir formato, orientação, conteúdo e atualização das prévias após substituir o original, além da preservação do arquivo em falhas.
