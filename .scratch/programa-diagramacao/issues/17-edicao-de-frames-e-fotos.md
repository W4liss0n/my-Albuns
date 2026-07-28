# 17 — Edição de Frames e Fotos

**What to build:** oferecer edição completa da geometria de vários Frames e do enquadramento de suas Fotos, mantendo a Foto sempre recortada pela máscara retangular e sem áreas vazadas.

**Blocked by:** 09 — Primeira composição com Foto; 10 — Álbum físico.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md).

- [ ] O usuário pode criar, selecionar, mover, redimensionar, sobrepor e ordenar vários Frames no Modo de edição da Lâmina.
- [ ] A Barra da Lâmina oferece uma ação de duas setas que troca entre as Páginas esquerda e direita os Frames e suas Fotos, mantendo a Numeração de Página.
- [ ] A troca translada Frames integralmente contidos para a mesma posição relativa no lado oposto, sem espelhar e preservando geometria, ajustes, estilo e ordem; Travessias centrais não mudam, Página única bloqueia, e tudo forma uma ação.
- [ ] A primeira versão permite manter vários Frames selecionados simultaneamente; a seleção é transitória, não participa de Undo/Redo e não faz parte do Projeto salvo.
- [ ] Ao entrar no Modo de edição, preservar o Frame selecionado somente quando pertencer à Lâmina isolada; ao sair com `Esc`, limpar toda a seleção.
- [ ] Clique simples substitui a seleção; `Ctrl` + clique adiciona ou remove um Frame; clique em área vazia limpa a seleção. A primeira versão não possui Caixa de seleção.
- [ ] Usar o limiar padrão de arraste da plataforma: ultrapassá-lo sobre um Frame selecionado preserva e move o grupo; sobre um não selecionado, seleciona e move somente esse Frame; soltar antes executa apenas o clique.
- [ ] Undo/Redo elimina da seleção referências a Frames removidos, mantém elementos ainda existentes e não volta a selecionar Frames restaurados.
- [ ] Em pontos com sobreposição, clique e `Ctrl` + clique atingem somente o Frame mais acima na Pilha visual; não implementar gesto de ciclo entre elementos encobertos.
- [ ] Na seleção múltipla, manter os contornos individuais e mostrar uma Caixa delimitadora única; arrastar qualquer selecionado move todos preservando distâncias, e as alças da Caixa redimensionam suas posições e dimensões proporcionalmente.
- [ ] Exibir oito alças: laterais escalam um eixo e cantos escalam os dois independentemente, sempre com o lado/canto oposto como âncora.
- [ ] Parar antes de inverter, ultrapassar a superfície ou reduzir algum Frame abaixo do mínimo calibrado.
- [ ] Aplicar em Frame único e grupo: `Shift` no canto preserva proporção, `Alt` em qualquer alça usa o centro e `Shift + Alt` combina os dois, respondendo dinamicamente durante o arraste.
- [ ] Quando um gesto coletivo ultrapassaria a superfície ativa, limitar o grupo inteiro no ponto válido mais próximo — superfície da Lâmina dupla ou somente Página ativa — sem ajustar Frames individualmente.
- [ ] Registrar cada movimento ou redimensionamento coletivo completo como uma única ação de Undo/Redo.
- [ ] Oferecer `Trazer para frente`, `Avançar uma posição`, `Recuar uma posição` e `Enviar para trás` no menu de contexto e em `Editar > Organizar`; usar `Ctrl` + `]`/`[` para avançar/recuar.
- [ ] Para avançar/recuar uma posição, mover cada bloco contíguo selecionado através do não selecionado adjacente; um bloco no limite não bloqueia os demais.
- [ ] Para os extremos, reunir todos os selecionados em um bloco; preservar sempre a ordem relativa de selecionados e não selecionados, em uma única ação.
- [ ] Aplicar qualquer comando de seleção a todos os elementos compatíveis como uma única ação, sem ignorar as restrições específicas.
- [ ] Em Layout destravado, `Delete`/`Excluir` remove sem confirmação todos os Frames selecionados e suas Fotos, preservando as geometrias restantes.
- [ ] Em Layout travado, `Delete`/`Excluir` remove somente as Fotos dos Frames selecionados e conserva suas geometrias como placeholders; o comando não move nem redimensiona Frames.
- [ ] Manter alinhamento e distribuição automáticos de Frames fora da primeira versão.
- [ ] Adicionar uma Foto cria normalmente um Frame com a Foto; a criação explícita sem Foto produz um Frame placeholder.
- [ ] `Editar > Adicionar Frame` e o menu de contexto vazio criam imediatamente um placeholder centralizado, proporcional à superfície ativa e selecionado, como uma ação única e sem modo de desenho.
- [ ] Arrastar uma Foto para um Frame preenchido ou placeholder afeta somente esse alvo, substituindo ou preenchendo sem mudar geometria e estilo.
- [ ] Em Frames sobrepostos, a Foto atinge somente o retângulo do Frame superior da Pilha visual, mesmo vazio ou transparente; não oferecer alternância para Frames inferiores.
- [ ] Arrastar para área vazia cria Frame proporcional centrado na soltura no Modo de edição e usa o primeiro Layout compatível no modo normal.
- [ ] No arraste de Foto, exibir somente destaque do alvo, nunca a Foto composta, o novo Frame ou o Layout futuro; cancelar sem criar Histórico.
- [ ] Dois cliques no Painel usam a Lâmina centralizada ou isolada e preenchem primeiro o placeholder mais à esquerda; sem placeholder, criam Frame com o primeiro Layout compatível no modo normal ou geometria centralizada proporcional no Modo de edição.
- [ ] Desempatar o placeholder mais à esquerda pela menor borda superior.
- [ ] Toda inserção ou substituição de Foto deixa somente o Frame afetado selecionado; a seleção não cria ação separada no Histórico.
- [ ] Fora do Frame nada da Foto aparece, e o Zoom base de Preenchimento do Frame impede vazios em qualquer proporção suportada.
- [ ] Fora do Modo de edição, `Alt` + clique e arraste sobre um Frame aplica Pan à Foto e `Alt` + roda aplica Zoom adicional, sem mover sua geometria.
- [ ] Esses gestos não selecionam o Frame: cada arraste é uma ação ao soltar, e passos consecutivos da roda são agrupados quando a sequência termina.
- [ ] No Modo de edição, gestos diretos selecionam, movem ou redimensionam Frames e não aplicam Pan/Zoom à Foto; ações estruturais indisponíveis no modo normal ficam claramente bloqueadas.
- [ ] Apagar um Frame destravado remove conjuntamente Frame e Foto; substituir a Foto preserva a geometria.
- [ ] Substituir uma Foto preserva Frame e estilo, mas reinicia a ocorrência centralizada, colorida, sem espelhamento, com Giro de 90° em `0°`, Ângulo em `0°` e Zoom do usuário em `1×`.
- [ ] Todo Frame permanece retangular, alinhado aos eixos e contido na área ativa; Travessia central só é permitida em Lâmina dupla.
- [ ] O mesmo Arquivo vinculado pode ocupar vários Frames, cada ocorrência com ajustes independentes, e cada Frame contém no máximo uma Foto.
- [ ] Toda edição participa de Undo/Redo, sobrevive a `Salvar`/reabrir e produz a mesma pilha e o mesmo recorte na Exportação JPEG.
- [ ] Testes cobrem zero, um e vários Frames, proporções extremas, sobreposição e placeholder manual.
