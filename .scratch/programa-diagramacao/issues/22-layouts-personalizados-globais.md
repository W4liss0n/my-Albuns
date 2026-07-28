# 22 — Layouts personalizados globais

**What to build:** permitir transformar a disposição atual de Frames em um Layout reutilizável em todos os Projetos, identificando automaticamente se sua organização pertence ao escopo de Lâmina ou Página.

**Blocked by:** 20 — Aplicação de Layouts.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] No Modo de edição da Lâmina, `Salvar disposição como Layout` aparece em `Design da Lâmina` e no menu `Editar`.
- [ ] A criação é imediata, não pede nome nem abre modal, mantém o Modo de edição e não aplica, trava ou modifica a composição atual.
- [ ] Persistir criação e exclusão imediatamente no catálogo global, sem marcar o Projeto aberto ou criar entrada em seu Undo/Redo.
- [ ] Armazenar o catálogo criado pelo usuário sob `%APPDATA%\MyAlbuns\Layouts`, fora de Projetos e do Cache descartável.
- [ ] Fazer `LayoutCatalogStore` serializar e publicar atomicamente cada alteração com schema e revisão monotônica; uma falha conserva a versão confirmada anterior e nunca substitui silenciosamente o catálogo por um vazio.
- [ ] Não exigir broadcast em tempo real no MVP. Cada Janela atualiza o catálogo ao abrir o Painel de Layouts, ao recuperar foco e por uma ação manual `Atualizar`, sem alterar composição, Salvamento ou Undo/Redo.
- [ ] Ao registrar ou reconectar uma Janela de Projeto, hidratá-la com o snapshot e a revisão vigentes; uma revisão mais nova invalida apenas a leitura local do catálogo.
- [ ] Exigir ao menos um Frame; sem Frames, desabilitar botão e item de menu com explicação curta.
- [ ] Se qualquer Frame atravessar o centro, o Layout é por Lâmina; se todos estiverem integralmente em um lado, o Layout é por Página, inclusive quando houver Frames nos dois lados.
- [ ] Para escopo de Página, cada Bloco de Frames é normalizado e centralizado globalmente na Página correspondente.
- [ ] O Layout guarda organização geométrica e metadados necessários, não Fotos, vínculos, estilos locais ou referência ao Projeto de origem.
- [ ] O Layout criado aparece entre os candidatos compatíveis de todos os Projetos.
- [ ] O Layout criado aparece na seção `Personalizados`, distinta da seção `Automáticos` produzida pelo Gerador de Layouts.
- [ ] Cada preview personalizada possui lixeira; excluir exige confirmação e remove somente o item do catálogo global.
- [ ] Alterar ou excluir o Layout global não modifica composições às quais ele já foi aplicado, Últimos Layouts aplicados ou Favoritos dos Projetos.
- [ ] A própria preview identifica o Layout; a interface apresenta compatibilidade e feedback de erro sem exigir nome ou classificação manual da disposição.
- [ ] A identidade de um Layout inclui escopo, tipo/proporção de superfície, quantidade e a sequência ordenada de posições/dimensões dos Frames; mudar a ordem semântica dos Frames produz outro Layout.
- [ ] Considerar duplicados somente Layouts cuja identidade completa, inclusive ordem dos Frames, seja igual.
- [ ] Ao tentar salvar duplicata, não alterar o catálogo; avisar sem modal e localizar/realçar a preview existente na próxima abertura compatível do Painel no modo normal.
- [ ] Testes cobrem Frames somente à esquerda, somente à direita, nos dois lados, com Travessia central e duas sequências diferentes sobre o mesmo conjunto geométrico, comprovando Mapeamentos distintos.
