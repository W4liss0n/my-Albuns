# 06 — Alterar DPI com Undo e Redo

**What to build:** permitir que a pessoa altere o DPI do Projeto aberto como uma ação criativa única, veja o novo valor imediatamente e possa desfazer ou refazer essa alteração sem persistir o arquivo antes de `Salvar`.

**Blocked by:** 03 — Criar um Projeto padrão no local escolhido.

**Type:** implementation

**Status:** resolved

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [Contrato do Arquivo de Projeto v1](../../../docs/design/0013-contrato-do-arquivo-de-projeto-v1.md); [contrato público de persistência](../../../docs/design/0015-contrato-publico-de-persistencia-do-project-core.md).

- [x] O editor apresenta o DPI corrente a partir da projeção autoritativa da Sessão do Projeto, sem valor fixo ou cópia canônica no frontend.
- [x] A pessoa consegue informar um DPI inteiro entre `1` e `1200`; valores fora do intervalo ou incompatíveis com os limites raster estruturais são recusados antes de qualquer mutação.
- [x] Concluir uma alteração válida envia uma única intenção consolidada ao `ProjectCore` e cria exatamente uma nova Revisão do Projeto.
- [x] A mesma projeção devolvida pelo núcleo atualiza imediatamente DPI, Revisão, mudanças pendentes e disponibilidade de Undo/Redo.
- [x] A interface pode manter texto, foco e feedback transitórios durante a edição, mas não altera diretamente o estado criativo nem antecipa uma Revisão documental.
- [x] Undo restaura o DPI anterior; Redo restaura a alteração desfeita; uma nova alteração depois de Undo descarta o ramo de Redo e usa uma Revisão ainda não utilizada naquela Sessão.
- [x] A alteração não muda dimensões físicas, composição, Pan ou enquadramentos e não modifica o Arquivo de Projeto antes de `Salvar`.
- [x] Seleção, navegação do Canvas e demais estados transitórios não entram no Histórico junto com o DPI.
- [x] Esgotar o intervalo seguro de Revisões recusa a ação antes da mutação e mantém a Sessão utilizável para leitura, Salvamento e Exportação.
- [x] Frontend, Host e testes atravessam somente a interface externa do `ProjectCore`; nenhuma subdivisão interna da Sessão é exposta pelo transporte.
- [x] Testes automatizados cobrem alteração válida, valor inválido, Undo, Redo, ramificação do Histórico, mudanças pendentes, projeção instantânea e ausência de escrita no arquivo.

## Comments

- O fluxo produtivo usa uma única intenção `SetDpi`, valida um Documento candidato no `ProjectCore` antes de trocar o estado e devolve a projeção autoritativa pela interface pública de `EditableProject`; o Host apenas delega.
- O draft, a validação sintática e o estado de aplicação permanecem transitórios na interface. Uma confirmação válida gera uma única mutação; a projeção devolvida atualiza DPI, Revisão, mudanças pendentes e Undo/Redo.
- O Histórico conserva a maior Revisão já atribuída, portanto uma ramificação depois de Undo descarta Redo sem reutilizar números. O esgotamento do intervalo seguro e qualquer DPI inválido preservam estado, Histórico e bytes persistidos.
- A duplicação temporária de dispatch causada pelo motor demonstrativo permanece deliberadamente para a contração do scaffolding no Ticket 11. Compartilhar o limite simples `1..1200` por nova infraestrutura também não se justificou neste corte; o Core continua sendo a autoridade e revalida tudo.
- Evidências finais: `npm run build`; 25 arquivos e 162 testes de interface com tolerância de `10 s` somente no comando devido à lentidão local de JSDOM; `npm run test:rust`; `npm run quality:rust`; contrato gerado conferido; `git diff --check`.
- A revisão independente final não encontrou lacuna de especificação nem violação documentada; os dois únicos julgamentos estruturais foram conscientemente mantidos pelos motivos acima.
