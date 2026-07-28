# 34 — Registro de comandos, atalhos e modificadores do MVP

**What to build:** manter IDs, descrições, contextos declarados e associações padrão do MVP em um `CommandCatalog` estável, sem centralizar o estado transitório ou o dispatch da interface.

**Blocked by:** 05 — Arquitetura de UI e interação.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [Configurações do aplicativo](../../../docs/design/0009-configuracoes-do-aplicativo.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Atribuir a cada comando um identificador estável e independente do texto exibido, com rótulo, contexto declarado e associação padrão no catálogo.
- [ ] Fazer menus, menus de contexto, dicas e testes consultarem os descritores; cada contexto da interface conserva foco, seleção, elegibilidade e dispatch locais.
- [ ] Registrar como atalhos fixos do MVP as combinações de teclado especificadas, inclusive `Ctrl + E` para `Abrir no Photoshop`; modificadores de gestos como `Alt` para Pan/Zoom da Foto e `Ctrl` para seleção múltipla permanecem definidos nos respectivos handlers da interface.
- [ ] Manter `Esc` reservado e não remapeável para cancelar a operação atual ou sair de um modo.
- [ ] Declarar contextos no catálogo, mas fazer a interface resolver localmente elegibilidade e conflitos: dois comandos simultaneamente elegíveis não podem responder ao mesmo evento.
- [ ] Manter clique, duplo clique, arraste, roda do mouse e seus modificadores nos módulos da interface que reconhecem esses gestos, sem criar um registro universal ou permitir personalização na primeira versão.
- [ ] Atualizar automaticamente rótulos de menus e dicas a partir do registro e preservar comportamento idêntico em todas as Janelas de Projeto.
- [ ] Manter o registro interno no MVP sem aba, lista de consulta, campos, restauração ou persistência de remapeamentos em Configurações.
- [ ] Estruturar IDs e resolução de contexto para permitir remapeamento futuro sem alterar os comandos de domínio; quando priorizado, implementar teclado antes de modificadores de gestos.
- [ ] Distinguir comandos de domínio, comandos de aplicação e ações somente da interface; Undo/Redo armazena deltas de domínio e nunca IDs genéricos do `CommandCatalog`.
- [ ] Testar unicidade de IDs, conflitos por contexto, indisponibilidade em modos incorretos, `Esc`, menus/dicas e execução simultânea em múltiplas Janelas.
- [ ] Personalização de atalhos, validação editável de conflitos, preferências em `settings.json` e remapeamento de modificadores ficam explicitamente fora do MVP.
