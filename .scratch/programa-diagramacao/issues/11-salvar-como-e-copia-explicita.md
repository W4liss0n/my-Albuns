# 11 — Salvar como e cópia explícita

**What to build:** permitir criar deliberadamente outro Projeto a partir do atual, copiando todo o estado visível e as preferências pertencentes ao Projeto sem estabelecer herança ou sincronização entre os dois.

**Blocked by:** 09 — Primeira composição com Foto.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0002 — identificar Cópias externas](../../../docs/adr/0002-identificar-copias-externas.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md).

- [ ] `Salvar como` abre diálogo nativo, aceita os caminhos totalmente qualificados do contrato Windows, preserva o Projeto original e cria um documento com nova identidade.
- [ ] A nova Identidade usa um namespace de Cache próprio e inicialmente vazio; representações e metadados descartáveis do original não são copiados.
- [ ] Depois do sucesso, encerrar o checkpoint de Recuperação da Identidade anterior; mudanças posteriores usam a nova Identidade. Cancelamento ou falha mantém a sessão, o Cache e a Recuperação anteriores.
- [ ] A cópia preserva estrutura, conteúdo, vínculos externos, Frames, transformações e demais dados já suportados.
- [ ] Cancelar, escolher destino relativo, namespace não suportado ou outro destino inválido, encontrar o Destino indisponível ou falhar ao gravar não troca o Projeto aberto nem deixa uma cópia parcialmente válida.
- [ ] Depois da criação, editar e salvar qualquer uma das cópias nunca modifica a outra.
- [ ] A interface deixa claro qual caminho e Nome pertencem ao Projeto atualmente aberto.
- [ ] Testes abrem os dois Projetos simultaneamente e comprovam isolamento de conteúdo, Undo/Redo e salvamento.
