# 15 — Ciclo de mídias externas

**What to build:** completar o ciclo de importação e manutenção de Fotos e Decorativos vinculados, oferecendo prévias rápidas, avisos de ausência e religação controlada sem incorporar os originais ao Projeto.

**Blocked by:** 09 — Primeira composição com Foto.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0001 — vincular Arquivos externos](../../../docs/adr/0001-vincular-arquivos-externos.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] A interface importa JPG/JPEG, PNG e TIFF/TIF para a aba ativa e rejeita formatos ou arquivos inválidos com motivo claro.
- [ ] A importação comum aceita seleção múltipla de arquivos ou uma pasta; ao escolher pasta, considera somente os arquivos diretamente nela e não percorre subpastas.
- [ ] O mesmo fluxo aceita arquivos e pastas arrastados do sistema operacional para o Painel, mantendo a aba ativa como destino e a regra não recursiva para pastas.
- [ ] Cada tentativa de Importação usa um `OperationPathContext` proprietário, aceita os formatos locais, UNC, unidade mapeada e caminhos longos do contrato e o congela em `RootBindingPlan` se distribuir trabalho; I/O de rede nunca executa na thread da interface.
- [ ] Falhas individuais não revertem uma importação múltipla: arquivos válidos entram, duplicatas não são erros e rejeições aparecem ao final na Tela de Problemas com `Arquivo` e `Motivo`.
- [ ] Cada seleção, pasta ou soltura forma uma ação de Undo/Redo somente com os vínculos novos, deixa mudanças pendentes e nunca altera originais ou duplicatas preexistentes; operação sem novidade não cria Histórico.
- [ ] O Projeto persiste vínculos externos e metadados necessários; os bytes originais não são incorporados.
- [ ] Prévias usam Cache reconstruível, respeitam orientação e são invalidadas quando o arquivo no mesmo caminho é substituído.
- [ ] Fazer `MediaMonitor` somente sinalizar alterações nos caminhos vinculados e consolidar eventos rápidos; quando o arquivo estiver estável, `MediaResolver` executa a inspeção autoritativa e devolve uma observação imutável.
- [ ] Depois da confirmação, fazer `MediaRuntime` registrar a observação, pedir ao `CacheEngine` a invalidação localizada e atualizar Painel, Frames e ocorrências abertas, evitando estados falsos durante uma gravação externa.
- [ ] Marcar ausência somente quando a raiz estiver acessível e o alvo não existir; restaurar o item sem Religação quando ele reaparecer no caminho registrado.
- [ ] Se rede, compartilhamento ou permissão impedirem confirmar existência, marcar `Arquivo indisponível`, preservar vínculo e prévia e oferecer `Tentar novamente` sem Undo/Redo, mudança pendente ou Religação.
- [ ] Uma atualização externa de conteúdo preserva a referência, não cria Undo/Redo e não marca o Projeto como alterado.
- [ ] Um Arquivo ausente aparece como tal no editor e oferece religação; outras ocorrências e outros Projetos não são alterados implicitamente.
- [ ] Na Religação, `MediaResolver` propõe candidatos, a interface confirma, `ProjectSession` aplica um único comando, `MediaResolver` reinspeciona, `MediaRuntime` registra a observação e `CacheEngine` reconstrói; somente a referência do Projeto atual muda, com Undo/Redo e Salvamento explícito.
- [ ] Na Exportação individual, `Relinkar` recebe uma pasta de Fotos, atualiza a sessão aberta pelas regras normais e permite usar as referências corrigidas sem Salvamento automático.
- [ ] No lote, `Relinkar todos` escolhe uma raiz e resolve temporariamente cada Projeto somente dentro de pasta com o mesmo Nome, sem regravar Projeto ou criar Undo/Redo.
- [ ] Procurar recursivamente pasta com Nome exato do Projeto e, dentro dela, nome e extensão exatos do arquivo; aceitar automaticamente apenas uma correspondência e manter zero ou múltiplas sem resolução.
- [ ] Arquivo ausente apenas no Painel gera aviso; somente um original efetivamente necessário à seleção bloqueia a Exportação.
- [ ] Importar o mesmo caminho na mesma aba evita duplicação conforme a identidade definida, enquanto o mesmo arquivo pode existir de forma independente nas duas abas.
- [ ] Não mesclar automaticamente dois vínculos de mídia apenas porque sua identidade física coincide; a equivalência física de Projeto e o bloqueio não redefinem a regra funcional de duplicação do Painel.
- [ ] Limpar ou perder o Cache apenas regenera prévias; nunca remove itens nem autoriza operação final sem o original.
- [ ] Preferências e mudanças do Projeto participam dos mecanismos corretos de persistência, e os fluxos críticos têm testes com arquivos temporários reais, incluindo alteração detectada pelo watcher.
