# 14 — Recuperação de sessão

**What to build:** recuperar alterações de uma sessão interrompida em um estado editável separado, preservando a regra de que o arquivo do usuário só muda quando ele executa `Salvar`.

**Blocked by:** 08 — Esqueleto ponta a ponta; 12 — Movimentação e Cópia externa.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0002 — identificar Cópias externas](../../../docs/adr/0002-identificar-copias-externas.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Alterações relevantes da sessão geram dados temporários de recuperação sem realizar salvamento automático do Projeto.
- [ ] Persistir esses dados pelo `RecoveryStore`, atomicamente e com schema sob `%LOCALAPPDATA%\MyAlbuns\Recovery\Projects\{project-id}.json`, separados de Cache, recuperação de lotes, arquivos de Projeto e mídia original.
- [ ] Atualizar a Recuperação depois de cada ação concluída, com postergação curta para consolidar ações próximas; gestos contínuos permanecem somente em memória e geram um único checkpoint ao terminar.
- [ ] Gravar cada checkpoint por substituição atômica, sem modificar o arquivo do Projeto; uma queda durante um gesto recupera o estado anterior ao início daquele gesto.
- [ ] Remover o temporário depois de `Salvar` sem novas mudanças ou de fechamento normal confirmado.
- [ ] Ao detectar recuperação disponível, a interface permite restaurar ou descartar de forma explícita.
- [ ] Isolar cada checkpoint por Identidade persistente e nunca recuperar estado de um Projeto em outro. Como cada Projeto possui host independente, a queda de um host não encerra as demais Janelas nem mistura seus checkpoints.
- [ ] O aviso oferece `Reabrir e recuperar`, `Abrir última versão salva` e `Agora não`.
- [ ] `Reabrir e recuperar` cria uma nova sessão ainda não salva; `Abrir última versão salva` exige confirmação antes de descartar o temporário; `Agora não` mantém a recuperação para a próxima abertura.
- [ ] Restaurar reconstitui somente o estado criativo consolidado, marca a sessão como não salva, inicia Undo/Redo vazios e não sobrescreve o arquivo original.
- [ ] O checkpoint contém o estado consolidado e o identificador/revisão da última versão salva usada como base; não persiste pilhas, comandos ou deltas de Undo/Redo.
- [ ] Nunca incluir pixels, Cache ou cópias de originais no checkpoint.
- [ ] Descartar remove somente os dados temporários confirmados, sem tocar no Projeto ou em mídia externa.
- [ ] Recuperações pertencem à identidade correta e não vazam entre Cópias externas.
- [ ] Depois de `Salvar como` bem-sucedido, remover o checkpoint da Identidade anterior; novas mudanças usam a nova Identidade, enquanto cancelamento ou falha preserva checkpoint e sessão anteriores.
- [ ] Testes simulam encerramento entre ações e no meio de um gesto, consolidação de ações rápidas, escrita interrompida, Histórico vazio depois da recuperação, outros Projetos abertos, as três escolhas, descarte confirmado, liberação de bloqueios e posterior `Salvar`.
