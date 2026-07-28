# 21 — Layout travado

**What to build:** permitir aplicar e travar um Layout diretamente pelo painel, preservando sua estrutura de Frames até que o usuário o destrave e impedindo Exportação enquanto houver posições sem Foto.

**Blocked by:** 20 — Aplicação de Layouts.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md).

- [ ] O usuário pode travar pelo controle de cadeado na preview do Layout; não existe uma tela separada para esse comando.
- [ ] Quando travada, a preview aplicada fica destacada com cadeado fechado e todas as demais previews ficam desabilitadas.
- [ ] Travar aplica o Layout escolhido e cria Frames placeholders para todas as posições que ainda não tenham Foto.
- [ ] Na navegação comum aparecem somente Layouts com a mesma quantidade; como candidatos de travamento, o Painel também oferece Layouts com mais posições. Layouts com menos posições são incompatíveis, e posições excedentes criam placeholders herdados.
- [ ] Enquanto estiver travada, a Lâmina não aceita outra geometria de Layout; é necessário destravar primeiro.
- [ ] O travamento congela quantidade, posição e dimensões dos Frames: impede criação/exclusão de estruturas, movimento e redimensionamento, enquanto seleção, ordem, Borda, Opacidade, Pan, Zoom, Giro, Ângulo, Espelhamento, efeitos e substituição de Foto continuam editáveis.
- [ ] A ação de Troca de lados da Barra da Lâmina fica indisponível enquanto o Layout estiver travado, pois alteraria as posições dos Frames.
- [ ] `Editar > Adicionar Frame`, o comando equivalente no menu de contexto e a soltura de Foto em área vazia ficam indisponíveis; dois cliques em uma Foto sem placeholder disponível não modificam o Projeto e orientam o usuário a arrastá-la para uma posição existente.
- [ ] Uma nova ocorrência de Foto só pode preencher placeholder; arrastar explicitamente sobre Frame preenchido substitui sua Foto sem alterar a estrutura.
- [ ] Apagar conteúdo de uma posição travada remove a Foto e mantém o Frame placeholder.
- [ ] A validação identifica cada placeholder dentro da seleção e bloqueia Exportação com motivo exato até que todos sejam preenchidos.
- [ ] O diálogo de problemas representa o bloqueio por placeholder com Projeto, motivo e ação `Abrir Projeto`.
- [ ] Depois de preencher os placeholders, a mesma seleção exporta normalmente.
- [ ] Clicar no cadeado fechado da preview destacada destrava sem diálogo, reabilita as demais previews e preserva posições, dimensões, ordem, Fotos, estilos, ajustes e placeholders existentes.
- [ ] Travar e destravar têm Undo/Redo, sobrevivem a `Salvar`/reabrir e mantêm a geometria exibida na Exportação JPEG.
- [ ] Testes verificam bloqueio de criação, exclusão estrutural, movimento, redimensionamento, Troca de lados e alvo vazio, além das edições de conteúdo, ordem, estilo e Foto que continuam permitidas.
