# 05 — Arquitetura de UI, mapa de telas e interação do editor

**What to build:** produzir o mapa navegável e o protótipo de interação que estabelecem a estrutura das telas, os modos do editor e os principais estados da primeira versão antes da implementação funcional.

**Blocked by:** 01 — Plataforma e arquitetura.

**Type:** design

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [designs de interface](../../../docs/design/); [ADR 0005 — Tauri, React e Rust](../../../docs/adr/0005-adotar-tauri-react-rust.md).

- [ ] Entregar um mapa de navegação cobrindo Boas-vindas, criação em duas etapas, Janela de Projeto, modo normal, Modo de edição, Exportação normal, Geração e Exportação em lote, Tela de Problemas, progresso e Configurações.
- [ ] Entregar um protótipo navegável de fidelidade suficiente para validar hierarquia, foco, seleção, hover, estados bloqueados, menus, diálogos, splitters e transições; estilo visual final não faz parte deste ticket.
- [ ] Representar a Boas-vindas como interface leve de `MyAlbuns.exe`, com Projetos recentes, Novo/Abrir Projeto, Exportação em lote, Configurações e Ajuda.
- [ ] Representar a criação em exatamente duas etapas, `Dimensões` e `Personalização`, seguida pelo diálogo nativo para Nome e Localização; a segunda etapa inclui reprodução viva de uma Lâmina dupla com Frames demonstrativos.
- [ ] Na Janela de Projeto, localizar menu desktop, Canvas horizontal contínuo de Lâminas, Painel de imagens abaixo, Painel contextual recolhível à direita e os splitters horizontal e vertical.
- [ ] No modo normal, mostrar todas as Lâminas interativas na mesma escala, sem Zoom; a Lâmina mais centralizada é o alvo implícito de `Enter` e de dois cliques no Painel de imagens.
- [ ] No Modo de edição, isolar e ampliar uma Lâmina, reduzir o Painel de imagens, permitir Zoom apenas por `Ctrl` + `+`, `Ctrl` + `−`, `Ctrl` + roda e reset por `Ctrl` + `0`; `Esc` retorna ao modo normal.
- [ ] Representar a Barra da Lâmina com números de Página, número da Lâmina, Troca de lados e controle do Painel de Layouts horizontal acima do Canvas.
- [ ] Demonstrar reordenação pela Barra e pela Grade com espaço reservado, fantasma, deslocamento intermediário, cancelamento e confirmação somente na soltura.
- [ ] Demonstrar o Painel contextual nos contextos Álbum, `Design da Lâmina` e Frame/Foto, com seções recolhíveis e linguagem de usuário; nunca exibir os estados internos `default` ou `custom`.
- [ ] Em `Design da Lâmina`, representar a miniatura espacial que seleciona esquerda, direita ou `Ambos os lados`; hover apenas realça e clique fixa a seleção. Página única mantém o lado desativado inerte.
- [ ] Demonstrar seleção múltipla de Frames somente por clique e `Ctrl` + clique, sem Caixa de seleção; seleção divergente mostra `—`, vazio ou `Múltiplos`, e a primeira edição aplica um valor absoluto a todos os elementos compatíveis.
- [ ] Demonstrar Pan/Zoom da Foto no modo normal com `Alt` + arraste e `Alt` + roda, e manipulação de Frames no Modo de edição, inclusive Layout travado sem alças de movimento ou redimensionamento.
- [ ] Organizar a Exportação normal em um modal com Escopo, Modo (`Por lâmina` ou `Por página`), Formato, Destino, resumo e qualidade somente para JPEG; a interface não promete rollback absoluto da pasta de destino.
- [ ] Representar a Tela de Problemas tabular e reutilizável, o progresso geral com `X/Y` ou estado indeterminado e as janelas dedicadas das duas operações em lote.
- [ ] Representar `Configurações` com as abas iniciais `Desempenho` e `Photoshop`; o registro de comandos do MVP é interno e não exige aba de Atalhos.
- [ ] Mostrar estados vazio, carregando, inválido, Arquivo ausente, Arquivo indisponível, bloqueado, operação em andamento, falha e cancelamento, além de ordem de foco e acesso por teclado para os fluxos críticos; indisponibilidade oferece `Tentar novamente`, não Religação.
- [ ] Usar [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md), [referência da Barra da Lâmina](../../../docs/assets/referencia-barra-da-lamina.png) e [referência do Painel de Layouts](../../../docs/assets/referencia-painel-de-layouts-horizontal.png) como referências conceituais, sem copiar seu estilo visual.
- [ ] Validar o protótipo com os fluxos principais e registrar decisões pendentes no ticket proprietário, sem resolvê-las implicitamente no desenho.
- [ ] Este ticket entrega mapa, wireframes/protótipo e especificação de interação. Não exige persistência real, renderização final, Exportação funcional, Cache, watcher, integração com Photoshop ou topologia definitiva de processos.
