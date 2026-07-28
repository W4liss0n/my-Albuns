# 33 — Integração com Photoshop

**What to build:** detectar uma instalação compatível do Adobe Photoshop e permitir abrir uma Foto do MyAlbuns no aplicativo externo.

**Blocked by:** 15 — Ciclo de mídias externas; 32 — Configurações globais e Cache.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [Configurações do aplicativo](../../../docs/design/0009-configuracoes-do-aplicativo.md); [ADR 0001 — vincular Arquivos externos](../../../docs/adr/0001-vincular-arquivos-externos.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Mostrar em `Configurações > Photoshop` todas as instalações detectadas e a disponibilidade da integração.
- [ ] Sem preferência válida, selecionar a versão mais recente; permitir escolher outra versão ou usar `Localizar Photoshop...` para indicar manualmente o executável.
- [ ] Persistir a instalação escolhida pelo `StateStore` em `%LOCALAPPDATA%\MyAlbuns\State\photoshop.json` como estado local da máquina entre Projetos e sessões.
- [ ] Fazer o store publicar essa preferência por substituição atômica e propagá-la pelo mecanismo versionado usado ao registrar e reconectar Janelas de Projeto.
- [ ] A ausência do Photoshop desabilita somente a integração e não bloqueia edição, Salvamento ou Exportação.
- [ ] `Abrir no Photoshop` aparece no menu de contexto da Foto no Painel e do Frame preenchido; `Ctrl + E` é o atalho fixo do MVP registrado pelo ticket 34.
- [ ] Habilitar o comando somente para exatamente uma Foto contextual; Seleção de Frames múltipla não abre arquivos em massa.
- [ ] Abrir sempre o Arquivo vinculado original pelos caminhos locais, UNC, unidade mapeada ou longos aceitos, nunca Cache, recorte do Frame ou versão com Pan, Zoom, Giro, Ângulo, Espelhamento ou efeitos incorporados.
- [ ] Tratar o watcher somente como indício; depois de inspeção autoritativa, atualizar `MediaRuntime`, invalidar pelo `CacheEngine` e refletir todas as ocorrências abertas sem Histórico ou mudanças pendentes.
- [ ] Consolidar os eventos da gravação do Photoshop e aguardar o original ficar estável e legível; ausência confirmada sob raiz acessível marca o item, retorno ao mesmo caminho o restaura automaticamente e indisponibilidade temporária preserva o vínculo com `Tentar novamente`.
- [ ] Se a instalação escolhida deixar de existir ou não puder ser iniciada, falhar sem modificar o Projeto e sem usar Cache como substituto; informar o problema e oferecer acesso à configuração para escolher ou localizar outra instalação.
- [ ] Nunca usar uma representação de Cache como substituta silenciosa do Arquivo vinculado.
- [ ] Cobrir detecção, abertura do original, retorno após edição externa e atualização de todos os Projetos abertos que compartilham o mesmo caminho.
