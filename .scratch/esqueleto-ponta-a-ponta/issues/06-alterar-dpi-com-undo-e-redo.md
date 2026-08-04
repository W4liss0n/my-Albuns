# 06 — Alterar DPI com Undo e Redo

**What to build:** permitir que a pessoa altere o DPI do Projeto aberto como uma ação criativa única, veja o novo valor imediatamente e possa desfazer ou refazer essa alteração sem persistir o arquivo antes de `Salvar`.

**Blocked by:** 03 — Criar um Projeto padrão no local escolhido.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [Contrato do Arquivo de Projeto v1](../../../docs/design/0013-contrato-do-arquivo-de-projeto-v1.md); [contrato público de persistência](../../../docs/design/0015-contrato-publico-de-persistencia-do-project-core.md).

- [ ] O editor apresenta o DPI corrente a partir da projeção autoritativa da Sessão do Projeto, sem valor fixo ou cópia canônica no frontend.
- [ ] A pessoa consegue informar um DPI inteiro entre `1` e `1200`; valores fora do intervalo ou incompatíveis com os limites raster estruturais são recusados antes de qualquer mutação.
- [ ] Concluir uma alteração válida envia uma única intenção consolidada ao `ProjectCore` e cria exatamente uma nova Revisão do Projeto.
- [ ] A mesma projeção devolvida pelo núcleo atualiza imediatamente DPI, Revisão, mudanças pendentes e disponibilidade de Undo/Redo.
- [ ] A interface pode manter texto, foco e feedback transitórios durante a edição, mas não altera diretamente o estado criativo nem antecipa uma Revisão documental.
- [ ] Undo restaura o DPI anterior; Redo restaura a alteração desfeita; uma nova alteração depois de Undo descarta o ramo de Redo e usa uma Revisão ainda não utilizada naquela Sessão.
- [ ] A alteração não muda dimensões físicas, composição, Pan ou enquadramentos e não modifica o Arquivo de Projeto antes de `Salvar`.
- [ ] Seleção, navegação do Canvas e demais estados transitórios não entram no Histórico junto com o DPI.
- [ ] Esgotar o intervalo seguro de Revisões recusa a ação antes da mutação e mantém a Sessão utilizável para leitura, Salvamento e Exportação.
- [ ] Frontend, Host e testes atravessam somente a interface externa do `ProjectCore`; nenhuma subdivisão interna da Sessão é exposta pelo transporte.
- [ ] Testes automatizados cobrem alteração válida, valor inválido, Undo, Redo, ramificação do Histórico, mudanças pendentes, projeção instantânea e ausência de escrita no arquivo.
