# 18 — Estilos e transformações

**What to build:** permitir personalizar aparência do Frame e transformações não destrutivas da Foto, distinguindo corretamente defaults do Projeto de substituições locais.

**Blocked by:** 17 — Edição de Frames e Fotos.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Configurações do Projeto definem borda padrão de Frame, incluindo presença, espessura e cor; a Borda é desenhada para dentro e preserva a geometria externa.
- [ ] `Design do Álbum > Padrão dos Frames` mostra uma prévia simples, `Exibir borda`, cor e espessura na Unidade do Projeto; mudanças imediatas atualizam Frames que usam o design do Álbum em uma ação de Undo/Redo.
- [ ] Opacidade não aparece no padrão global e continua sendo exclusivamente individual.
- [ ] Qualquer alteração manual de Borda ou Opacidade torna o estilo inteiro do Frame `custom`; a Opacidade afeta conjuntamente Foto e Borda. Restaurar o padrão reaplica a Borda atual, redefine a Opacidade para `100%` e retoma a herança.
- [ ] A Foto aceita preto e branco, espelhamento horizontal, Giro sucessivo de 90 graus no sentido anti-horário e Ângulo fino contínuo entre `-45°` e `+45°`, armazenado separadamente do Giro.
- [ ] `Ajustes e Efeitos` expõe somente `Preto e branco` na primeira versão; brilho, contraste, saturação e filtros futuros não aparecem como controles desabilitados.
- [ ] Ângulo usa slider e entrada numérica com precisão de `0,1°`; dois cliques no slider restauram `0°`, sem botão de reset.
- [ ] Transformações são não destrutivas e mantêm separado o Zoom base necessário para preencher do Zoom adicional do usuário.
- [ ] Persistir Pan, Zoom adicional, espelhamento, Giro e Ângulo como `MediaTransform`; manter o Zoom e o deslocamento do Canvas como `ViewportTransform` somente da interface, sem Salvamento, Undo/Redo ou Exportação.
- [ ] No modo normal, `Alt` + arraste sobre o Frame aplica Pan e `Alt` + roda aplica Zoom da Foto sob o ponteiro; no Modo de edição, esses gestos diretos não editam a Foto.
- [ ] Pan/Zoom direto preserva a seleção; um arraste completo e uma sequência contínua da roda geram cada qual uma única ação de Undo/Redo.
- [ ] A ordem normativa é Giro de 90°, Ângulo, Espelhamento horizontal, Zoom de preenchimento, Zoom do usuário, Pan e Efeitos; alterar Giro, Ângulo ou Espelhamento recalcula preenchimento e limites de Pan sem revelar áreas vazias.
- [ ] Redimensionar o Frame preserva proporcionalmente Pan, Zoom do usuário e ponto focal, limitando-os apenas para impedir vazamentos.
- [ ] Quando houver vários Frames selecionados, o Painel contextual mostra as quantidades de Frames, Fotos e placeholders, não elege uma Foto para preview e exibe somente controles aplicáveis em lote.
- [ ] Borda e Opacidade afetam todos os Frames selecionados, inclusive placeholders; Zoom, Ângulo, Giro de 90°, Espelhamento e Preto e branco afetam somente os selecionados que contêm Foto, com indicação da quantidade atingida e controles de Foto ocultos quando nenhuma estiver selecionada.
- [ ] Valores numéricos divergentes aparecem como estado indeterminado (`—`, campo vazio ou `Múltiplos`), propriedades binárias divergentes usam estado neutro e cores diferentes usam amostra vazia; esses estados não modificam os elementos.
- [ ] O primeiro ajuste explícito em um controle divergente aplica o mesmo valor absoluto a todos os elementos compatíveis como uma única ação de Undo/Redo.
- [ ] Mudanças têm controles visíveis, Undo/Redo, persistem após reabertura e coincidem exatamente na Exportação JPEG.
- [ ] Testes verificam combinações de transformações, mudança de proporção do Frame, ausência de área vazada e edição em lote com valores iguais, divergentes, Fotos e placeholders.
