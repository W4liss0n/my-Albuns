# 19 — Painel de imagens

**What to build:** entregar o Painel de imagens completo, no qual Fotos e Decorativos têm importação, organização, filtros e indicadores de uso próprios sem misturar responsabilidades.

**Blocked by:** 16 — Background e Overlay; 17 — Edição de Frames e Fotos.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md); [ADR 0001 — vincular Arquivos externos](../../../docs/adr/0001-vincular-arquivos-externos.md).

- [ ] O painel possui abas `Fotos` e `Decorativos`; uma importação entra na aba que estiver ativa.
- [ ] A busca filtra em tempo real pelo Nome do arquivo somente na aba ativa e ignora diferenças entre maiúsculas, minúsculas e acentos.
- [ ] Combinar a busca por interseção com os filtros ativos sem alterar a ordenação dos resultados; um `X` limpa o texto da aba atual.
- [ ] Manter um texto de busca independente por aba enquanto a Janela do Projeto estiver aberta, sem persistir na sessão seguinte, alterar o Projeto ou participar de Undo/Redo.
- [ ] Um slider único ajusta continuamente o tamanho das miniaturas da aba ativa e reorganiza a grade em tempo real; dois cliques restauram o tamanho médio padrão.
- [ ] Preservar a proporção inteira de cada miniatura sem corte e calibrar no protótipo os tamanhos mínimo, máximo e médio.
- [ ] Manter tamanhos independentes para `Fotos` e `Decorativos` como preferências globais reutilizadas entre Projetos e sessões, sem alterar o Projeto ou o Histórico.
- [ ] Clique simples substitui a seleção e estabelece a âncora; `Ctrl` + clique adiciona ou remove uma mídia individualmente.
- [ ] `Shift` + clique seleciona o intervalo contínuo entre a âncora e o item acionado conforme a ordem visível depois de busca, filtros e ordenação.
- [ ] `Ctrl + A` seleciona exclusivamente todos os itens visíveis na aba ativa; itens ocultos pela busca ou pelos filtros nunca entram na seleção.
- [ ] Retirar imediatamente da seleção itens e eventual âncora que se tornem invisíveis após mudar busca ou filtro, preservando os demais selecionados visíveis.
- [ ] Alterar apenas a ordenação mantém os mesmos itens e a mesma âncora selecionados em suas novas posições.
- [ ] Clique direito sobre item selecionado preserva o grupo; sobre item não selecionado, substitui a seleção somente por ele antes de abrir o menu.
- [ ] Com foco no Painel, `Delete` e `Remover` no menu de contexto atuam sobre a seleção resultante; fora dele, o atalho obedece ao contexto do Canvas.
- [ ] Não implementar Caixa de seleção no Painel; arraste e duplo clique atuam somente sobre a mídia diretamente acionada, nunca sobre toda a seleção.
- [ ] A seleção do Painel é estado transitório da interface e não integra o Projeto ou Undo/Redo.
- [ ] A remoção atua sobre toda a seleção da aba como uma única operação; diálogos necessários são consolidados e nunca repetidos por item.
- [ ] `Importar` oferece `Arquivos...`, com seleção múltipla no diálogo do Windows, e `Pasta...`, que considera somente imagens diretamente contidas na pasta escolhida, sem busca recursiva.
- [ ] Aceitar arquivos e pastas arrastados do sistema operacional em toda a área do Painel; importar na aba ativa e tratar pastas sem percorrer subpastas.
- [ ] Em importação múltipla, aceitar todos os arquivos válidos sem reverter por falhas individuais; duplicatas não contam como erro.
- [ ] Havendo rejeições, abrir ao final a Tela de Problemas com `Arquivo` e `Motivo`, preservando os itens importados ao fechar.
- [ ] Agrupar todos os novos vínculos de uma seleção, pasta ou única soltura em uma ação de Undo/Redo que deixa mudanças pendentes; desfazer remove somente esses itens e refazer os restaura.
- [ ] Nunca alterar arquivos originais ou duplicatas preexistentes pelo Undo da importação; sem item novo, não criar Histórico nem marcar o Projeto.
- [ ] O seletor de padrão visual reutiliza somente itens já presentes em `Decorativos`; ele não cria um segundo fluxo de importação e não aceita arraste.
- [ ] Dois cliques em uma Foto usam a Lâmina centralizada ou isolada, preenchem primeiro o placeholder mais à esquerda e só criam novo Frame quando não houver placeholder.
- [ ] Determinar esse placeholder pela menor borda esquerda e, em empate, pela menor borda superior.
- [ ] Arrastar sobre Frame preenchido ou placeholder afeta exclusivamente esse alvo; Layout travado sem placeholder recusa o duplo clique e orienta o arraste para um placeholder.
- [ ] Em sobreposição, destacar e soltar somente no Frame mais acima cujo retângulo contém o ponteiro, independentemente de conteúdo ou Opacidade, sem seletor alternativo.
- [ ] Soltar em área vazia usa o primeiro Layout compatível no modo normal e, no Modo de edição, cria Frame proporcional centralizado no ponto e deslocado para dentro dos limites; Layout travado rejeita o alvo vazio.
- [ ] Durante o arraste de Foto, destacar apenas o Frame ou a Lâmina alvo, sem preview de conteúdo, novo Frame ou Layout; `Esc` e soltura inválida cancelam sem efeitos.
- [ ] Depois da operação, selecionar somente o Frame afetado e abrir seu contexto, sem outra ação de Undo/Redo.
- [ ] Dois cliques em um Decorativo aplicam-no como Background a `Ambos os lados` da Lâmina usada como alvo implícito; `Shift` + dois cliques aplicam-no como Overlay. Um lado individual só pode ser escolhido por arraste.
- [ ] Arrastar um Decorativo aplica Background sem modificador e Overlay enquanto `Shift` estiver pressionado; o Canvas mostra em tempo real o papel, o escopo e o próprio Decorativo composto no destino.
- [ ] O preview não altera o Projeto ou o Histórico; `Esc` e soltura fora de alvo válido cancelam o gesto sem efeitos.
- [ ] As zonas esquerda, central e direita aplicam o Decorativo respectivamente ao lado esquerdo, a `Ambos os lados` ou ao lado direito; em Página única, somente o lado ativo é alvo de arraste e a zona central não aparece.
- [ ] Dois cliques em Decorativo sobre o alvo implícito de Página única ainda criam uma aplicação de `Ambos os lados`, que poderá se expandir se a estrutura for convertida para Lâmina dupla.
- [ ] A zona central é uma faixa proporcional visível durante o arraste, e não somente a linha de junção; a largura final é validada no protótipo.
- [ ] Frames e Fotos não interceptam o arraste de Decorativos nem têm sua seleção alterada; o destino continua sendo a zona da Lâmina subjacente.
- [ ] A soltura lateral preserva o outro lado mesmo quando exige dividir uma aplicação inteira; a soltura central substitui ambos os lados por uma aplicação inteira, e cada gesto gera uma única ação de Undo/Redo.
- [ ] Itens podem ser ordenados por Nome natural, data de criação e data de alteração do arquivo original, em ordem crescente ou decrescente.
- [ ] Arquivos ausentes ficam identificados e agrupados ao fim, sem quebrar ordenação dos itens disponíveis.
- [ ] Cada aba possui filtro independente para `Usadas`, `Não usadas` e todas, calculado a partir do uso real no Projeto.
- [ ] O filtro adicional `Ausentes` pode ser ativado pelo Painel ou pelo aviso em Informações do Álbum; as abas mostram badges com suas respectivas quantidades de originais ausentes.
- [ ] Quando aberto pelo aviso, `Ausentes` guarda aba e filtros anteriores, restaura-os ao encerrar e não é persistido como preferência entre sessões.
- [ ] Indicadores distinguem usos em Frames, Backgrounds, Overlays e padrões do Álbum quando pertinente, sem expor termos internos de herança.
- [ ] Ordenação, filtro, aba e tamanho das miniaturas são preferências de interface persistentes que não sujam nem alteram o Projeto; o texto de busca permanece apenas durante a Janela aberta.
- [ ] Estados vazio, carregando e importando são representados no painel; falhas parciais são encaminhadas à Tela de Problemas ao concluir.
- [ ] Testes cobrem nomes numéricos, datas do original, itens ausentes, múltiplos usos, busca sem distinção de caixa ou acento, interseção com filtros, troca de abas, redimensionamento sem corte, seleção individual, por intervalo e por `Ctrl + A`, ocultação, reordenação, clique direito, foco do `Delete` e preferências entre sessões.
