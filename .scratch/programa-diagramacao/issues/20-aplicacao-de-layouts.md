# 20 — Aplicação de Layouts

**What to build:** permitir explorar e aplicar organizações compatíveis de Frames sem manter referência viva ao Layout original, preservando conteúdo e oferecendo automação previsível fora do Modo de edição.

**Blocked by:** 18 — Estilos e transformações.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0008 — arranjo de reserva de Layout](../../../docs/adr/0008-garantir-layout-compativel-por-arranjo-de-reserva.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Usar um conjunto versionado e determinístico de Layouts de teste como fonte de candidatos; o Gerador de Layouts do ticket 06 permanece adiado e não bloqueia este fluxo.
- [ ] O painel lista somente Layouts compatíveis com a quantidade atual de Frames, proporção e escopo aplicável.
- [ ] Cada Lâmina possui uma Barra própria; seu controle central abre ou fecha o único Painel de Layouts horizontal usando aquela Lâmina como alvo explícito.
- [ ] Barra e Painel de Layouts existem somente no modo normal e não aparecem nem aceitam comandos no Modo de edição.
- [ ] Suspender uma faixa aberta durante o Modo de edição e restaurá-la no mesmo alvo com candidatos recalculados ao sair; não abrir uma faixa antes fechada.
- [ ] A faixa ocupa a coluna de trabalho acima do Canvas e desloca as Lâminas sem cobri-las; escolher outra Barra reutiliza a faixa e troca o alvo, e o Painel contextual não contém Layouts.
- [ ] A faixa separa as previews em `Automáticos`, fornecidos no MVP pelo conjunto versionado de teste, e `Personalizados`, criados pelo usuário e disponíveis globalmente.
- [ ] Hover reorganiza transitoriamente os Frames reais com Fotos, placeholders, estilos, ordem e ajustes; candidatos de travamento mostram posições excedentes como placeholders vazios transitórios com Padrão de Frame herdado; saída restaura, outra preview substitui e nenhum estado/Histórico muda.
- [ ] `LayoutRules` recebe valores imutáveis e devolve um `LayoutPatch` para preview ou confirmação; hover nunca aplica o patch, e somente `ProjectSession` o confirma como um único comando.
- [ ] `LayoutRules` nunca devolve conjunto vazio: quando nenhum candidato do catálogo, dos favoritos ou das fixtures servir, produz o arranjo de reserva determinístico do ADR 0008, derivado apenas da quantidade de Frames e da superfície ativa.
- [ ] O arranjo de reserva atende às automações, mas não é item de catálogo: não recebe preview própria no Painel de Layouts, não pode ser favoritado e não vira Último Layout aplicado.
- [ ] Teste de contrato percorre toda quantidade suportada de Frames nos dois escopos e em ambos os formatos de superfície, com o catálogo e as fixtures vazios, exigindo ao menos um candidato em todos os casos.
- [ ] Clique confirma exatamente a prévia como uma ação; cadeado confirma o mesmo resultado, cria os placeholders excedentes e trava como uma ação.
- [ ] Em candidato com posições excedentes, desabilitar a aplicação pelo corpo da preview e aceitar a confirmação somente pelo cadeado.
- [ ] Previews automáticas mostram estrela e cadeado; previews personalizadas acrescentam a lixeira.
- [ ] Em cada seção, ordenar Último Layout aplicado compatível, favoritos do Projeto e demais candidatos, mantendo uma única preview por definição.
- [ ] Aplicar um Layout copia posições e dimensões para o Projeto sem criar dependência que permita à origem alterar a composição depois.
- [ ] Frames são mapeados pela ordem atual na Pilha visual e preservam Foto ou ausência, estilo e todos os ajustes; somente a geometria é substituída.
- [ ] Depois da aplicação, o usuário pode editar livremente os Frames; ao reabrir o painel, o Layout aplicado aparece primeiro sem ser tratado como referência viva.
- [ ] O Último Layout aplicado conserva separadamente sua geometria original, permanece primeiro enquanto compatível e pode ser reaplicado após ajustes manuais ou remoção da origem.
- [ ] Toda escolha automática usa a prioridade global: Último Layout aplicado compatível, primeiro favorito, primeiro personalizado e primeiro Layout de teste automático.
- [ ] Fora do Modo de edição e sem Layout travado, adicionar ou remover Foto/Frame aplica automaticamente o primeiro Layout da nova quantidade.
- [ ] Dentro do Modo de edição, adicionar ou remover não reorganiza automaticamente a composição.
- [ ] Em Lâminas de Página única, a automação usa o primeiro Layout de Página e não cria conteúdo no lado desativado.
- [ ] Aplicação e automação têm Undo/Redo, persistem após reabertura e resultam na mesma geometria na Exportação JPEG.
- [ ] Fixtures e sua interface de consulta reproduzem o contrato esperado do futuro Gerador, permitindo substituí-las sem mudar o documento de Projeto nem as regras de aplicação.

## Comments

- 2026-07-28: o ADR 0008 tornou absoluta a garantia de ao menos um Layout compatível e a atribuiu a `LayoutRules`, não ao Gerador. Isso acrescentou três critérios aqui e desbloqueou este ticket em relação ao 06: as automações podem ser construídas antes de o algoritmo do Gerador existir.
