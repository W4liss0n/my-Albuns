# 12 — Movimentação e Cópia externa

**What to build:** reconhecer quando o arquivo de Projeto foi apenas movido e, quando uma cópia feita pelo sistema operacional for aberta, atribuir-lhe identidade própria sem exigir uma ação adicional do usuário.

**Blocked by:** 08 — Esqueleto ponta a ponta.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0002 — identificar Cópias externas](../../../docs/adr/0002-identificar-copias-externas.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md).

- [ ] Abrir o mesmo documento em outro caminho após uma movimentação confirmada atualiza sua localização sem duplicar sua identidade e reutiliza seu namespace de Cache.
- [ ] Abrir o mesmo arquivo por unidade mapeada, UNC ou outra representação fisicamente equivalente não caracteriza movimentação nem Cópia externa e focaliza a sessão existente.
- [ ] Abrir dois arquivos fisicamente diferentes que descendem da mesma identidade persistida detecta a Cópia externa e promove uma nova identidade para ela.
- [ ] Se a identidade física for inconclusiva, ou se a localização anterior estiver indisponível em vez de comprovadamente ausente, não inferir movimentação ou Cópia externa e falhar de forma fechada.
- [ ] Ao detectar uma Cópia externa, o programa atribui e persiste automaticamente uma nova Identidade no arquivo copiado; essa escrita técnica nunca inclui alterações criativas pendentes.
- [ ] Resolver e persistir essa nova Identidade antes de acessar Cache ou Recuperação; a cópia começa em namespaces próprios e nunca lê ou escreve temporariamente nas pastas do original.
- [ ] Se o arquivo copiado não for gravável, não promover identidade apenas em memória e não montar Cache ou Recuperação sob a identidade duplicada; bloquear a abertura editável e oferecer `Salvar cópia como...` para um destino gravável.
- [ ] A interface explica apenas situações que exigem ação; a cópia normal não é punida com um fluxo desnecessário.
- [ ] Testes cobrem mover, copiar, renomear, aliases UNC/unidade mapeada, identidade inconclusiva, rede indisponível, abertura simultânea, edição isolada e Cópia externa somente leitura sem tocar nos namespaces do original.
