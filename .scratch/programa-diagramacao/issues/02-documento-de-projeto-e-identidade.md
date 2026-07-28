# 02 — Documento de Projeto e identidade

**What to build:** especificar o documento persistido de Projeto e o mecanismo que permite salvar com segurança, distinguir movimentações de Cópias externas, coordenar aberturas simultâneas e recuperar sessões sem salvamento automático.

**Blocked by:** 01 — Plataforma e arquitetura; 37 — Política e resolução de caminhos Windows.

**Type:** design

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0002 — identificar Cópias externas](../../../docs/adr/0002-identificar-copias-externas.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Registrar em ADR o formato e a extensão do arquivo de Projeto, sua identificação de tipo e a política de evolução de schema.
- [ ] Especificar a interface externa do `ProjectCore` mantendo `ProjectDomain`, `ProjectSession` e `ProjectStore` como responsabilidades internas: domínio sem I/O, sessão como única proprietária mutável e store como proprietário de formato, migração e Salvamento atômico.
- [ ] Definir identidade persistente, identidade da instância de arquivo e evidências usadas para distinguir movimentação, `Salvar como` e Cópia externa.
- [ ] Definir a codificação reversível usada pelo documento para os caminhos Windows aceitos, sem conversão com perda, normalização textual ou uso do caminho como Identidade.
- [ ] Tratar equivalência física como `Same`, `Different` ou `Indeterminate`: aliases da mesma instância focalizam a sessão existente; somente `Different` com a mesma Identidade caracteriza Cópia externa; `Indeterminate` falha de forma fechada.
- [ ] Usar uma representação opaca, estável e segura para caminho da Identidade persistente como namespace do Cache, sem derivá-la do Nome ou do caminho do arquivo.
- [ ] Resolver uma Cópia externa e persistir sua nova Identidade antes de montar, ler ou gravar qualquer estado local por Projeto, inclusive Cache e Recuperação, impedindo acesso temporário às pastas do original.
- [ ] Se a Cópia externa não puder receber a nova Identidade por estar somente leitura ou sem permissão de escrita, falhar de forma fechada: não abrir uma sessão editável nem montar Cache ou Recuperação sob a identidade duplicada; oferecer `Salvar cópia como...` e orientação acionável.
- [ ] Definir no `ProjectStore` o Salvamento atômico, revisão esperada e comportamento diante de falha, interrupção ou cancelamento do diálogo nativo, sem duplicar invariantes do domínio.
- [ ] Diferenciar `Arquivo ausente`, confirmado somente sob raiz acessível, de `Arquivo indisponível`, preservando localização e Identidade quando rede ou permissão não permitem concluir.
- [ ] Definir Bloqueio de abertura, detecção de bloqueio órfão e reutilização da sessão já aberta.
- [ ] Separar claramente o arquivo salvo dos dados temporários de Recuperação de sessão; nenhuma recuperação pode alterar o Projeto sem `Salvar`.
- [ ] A Recuperação usa substituição atômica fora do arquivo do Projeto e é removida quando não existem mudanças pendentes após `Salvar` ou fechamento normal.
- [ ] Garantir que mídia continue vinculada a caminhos externos e que qualquer Projeto copiado seja independente dos demais.
- [ ] Fornecer exemplos versionados de documento válido, migração e estados inválidos para orientar testes futuros.
