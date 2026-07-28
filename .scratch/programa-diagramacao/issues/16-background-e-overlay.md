# 16 — Background e Overlay

**What to build:** permitir compor cada Lâmina com Background e Overlay por lado ou em `Ambos os lados`, usando defaults do Projeto e substituições customizadas que respondem corretamente a mudanças posteriores do padrão.

**Blocked by:** 10 — Álbum físico; 15 — Ciclo de mídias externas.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md); [criação de Projeto](../../../docs/design/0003-criacao-de-projeto.md).

- [ ] A criação e as configurações do Projeto permitem escolher Background e Overlay padrão, por cor ou Decorativo compatível.
- [ ] Durante a criação, `Escolher imagem...` usa o seletor nativo e mantém o arquivo provisório na prévia; somente uma criação concluída o vincula à aba `Decorativos`, enquanto cancelamento não importa nada.
- [ ] A etapa `Personalização` compõe Background e Overlay imediatamente em uma reprodução demonstrativa da Lâmina, sem transformar a reprodução em conteúdo do Projeto.
- [ ] Na criação, a reprodução usa o mesmo hover/clique espacial de `Design do Álbum` para selecionar esquerda, direita ou `Ambos os lados`; somente o clique fixa o escopo dos controles.
- [ ] `Padrões visuais` apresenta uma miniatura exclusiva do padrão global; hover e clique selecionam esquerda, direita ou `Ambos os lados`, e os controles abaixo atuam nesse escopo.
- [ ] Em `Design do Álbum > Padrões visuais`, clicar no preview abre um seletor compacto de Decorativos já importados; escolher um item atualiza o padrão e as aplicações herdadas como uma única ação.
- [ ] O seletor de padrão não aceita arraste nem importa arquivos; a importação permanece no Painel de imagens.
- [ ] Cada Lâmina aceita escopo de lado individual ou `Ambos os lados`; um elemento aplicado a ambos não pode ser movido entre lados.
- [ ] Arrastar um Decorativo sem modificador aplica-o como Background; manter `Shift` pressionado durante o gesto aplica-o como Overlay.
- [ ] A soltura no lado esquerdo ou direito aplica ao respectivo lado; a região central aplica a `Ambos os lados` ativos da mesma Lâmina.
- [ ] O alvo de `Ambos os lados` é uma faixa proporcional ao redor da junção, exibida durante o arraste; sua largura exata é calibrada no protótipo e não exige acertar a linha central.
- [ ] Dois cliques em um Decorativo aplicam-no como Background a `Ambos os lados` da Lâmina centralizada, enquanto `Shift` + dois cliques aplicam-no como Overlay; no Modo de edição, usam a Lâmina isolada.
- [ ] A aplicação a um único lado exige arrastar para o alvo esquerdo ou direito.
- [ ] Frames e Fotos não interceptam o arraste de Decorativos; o alvo permanece a zona da Lâmina, sem substituir a Foto, preencher o Frame ou alterar a seleção.
- [ ] O feedback de arraste identifica papel e escopo antes da soltura e reage imediatamente quando `Shift` é pressionado ou solto.
- [ ] O próprio Decorativo aparece temporariamente no destino correto da Pilha visual, preservando transparência de Overlay e substituindo no preview somente o uso que seria alterado.
- [ ] O preview não altera Projeto, estado de Salvamento ou Histórico; `Esc` e soltura inválida cancelam sem Undo, enquanto somente uma soltura válida cria o comando.
- [ ] Em uma Lâmina de Página única, o lado desativado não é alvo, a faixa central não aparece e arrastar sobre a Página ativa cria uma aplicação daquele lado.
- [ ] Dois cliques em Página única ainda criam uma aplicação de `Ambos os lados`, limitada visualmente ao lado ativo e expansível se a estrutura for convertida para Lâmina dupla.
- [ ] A aplicação manual cria um `custom` apenas no papel e escopo atingidos e entra no Histórico como uma única ação.
- [ ] Soltar em um lado quando existe aplicação de `Ambos os lados` altera somente o alvo; o lado oposto preserva conteúdo, origem de herança e resultado visual sem reesticar a imagem.
- [ ] Soltar na região central substitui os dois lados daquele papel por uma única aplicação `custom` de `Ambos os lados`, como uma única ação de Undo/Redo.
- [ ] O estado `default` herda mudanças posteriores do Projeto, enquanto `custom` preserva o valor escolhido manualmente.
- [ ] Remover localmente um Overlay herdado cria uma ausência personalizada; remover localmente um Background herdado cria um Background branco personalizado; ambos permanecem `custom` até a Restauração do padrão.
- [ ] Personalizar somente um lado de uma aplicação herdada para `Ambos os lados` preserva o outro lado em `default`; mudanças posteriores de conteúdo ou escopo do Padrão visual afetam apenas a parte herdada.
- [ ] Remover um Decorativo customizado que não seja padrão restaura a propriedade para o default atual do Projeto.
- [ ] Background é renderizado abaixo dos Frames e Overlay acima; ambos são esticados/achatados para a superfície correta no comportamento inicial.
- [ ] `Design da Lâmina`, no Painel contextual direito durante o Modo de edição sem Frame/Foto selecionado, começa por uma miniatura real e atualizada da Lâmina.
- [ ] Hover em um lado da representação realça somente esse lado; hover na região central realça ambos; clicar mantém o escopo selecionado sem alterar a composição.
- [ ] Em Página única, o lado desativado da miniatura é neutro e inerte; somente a Página ativa é selecionável e a região central não cria seleção de `Ambos os lados`.
- [ ] A seleção inicial é `Ambos os lados` em Lâmina dupla e Página ativa em Página única; o último escopo retorna após uma seleção temporária de Frame/Foto na mesma sessão de edição.
- [ ] A seleção do esquema é descartada ao sair do Modo de edição e não participa do Projeto, Salvamento ou Undo/Redo.
- [ ] Background e Overlay aparecem abaixo da representação e atuam no escopo selecionado, com ações de remoção ou retorno ao design do Álbum.
- [ ] Miniatura e controles não aceitam Decorativos arrastados; imagens de Background e Overlay são aplicadas pelos gestos definidos no Canvas.
- [ ] `Background` mostra preview atual, seletor de cor, `Remover` e `Voltar ao design do álbum`; `Overlay` mostra preview ou `Sem overlay`, `Remover` e a ação de retorno.
- [ ] Escolher uma cor cria uma definição local no escopo selecionado e uma única ação de Undo/Redo; escolher imagens continua sendo feito no Canvas.
- [ ] Mostrar sempre `Usando o design do álbum` ou `Definido nesta lâmina`; exibir a ação de retorno somente no segundo caso e manter `Remover` disponível para afetar apenas o escopo selecionado.
- [ ] Com `Ambos os lados` selecionado e valores laterais diferentes, mostrar previews `Esquerda` e `Direita`, cada um com sua origem; uma ação afeta ambos em uma única entrada de Undo/Redo.
- [ ] Painéis e previews mostram escopo e usam `Usando o design do álbum`, `Definido nesta lâmina` e `Voltar ao design do álbum`, sem expor os termos internos `default` e `custom`; alterações têm Undo/Redo, sobrevivem a `Salvar`/reabrir e coincidem na Exportação JPEG.
- [ ] Lados desativados de Página única não recebem nem exibem propriedades próprias.
- [ ] Testes cobrem as três zonas de soltura, mudança do modificador durante o arraste, preview nas duas camadas, cancelamento por `Esc` e soltura inválida, limites entre zonas, passagem sobre Frames e Fotos, Lâmina de Página única e conversões nos dois sentidos entre escopo inteiro e escopos laterais.
