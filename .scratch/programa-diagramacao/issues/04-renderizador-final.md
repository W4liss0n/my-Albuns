# 04 — Renderizador final

**What to build:** especificar um pipeline determinístico que componha o estado atual do Projeto usando os originais e produza arquivos finais nas dimensões físicas esperadas, sem depender do Canvas ou do Cache de visualização.

**Blocked by:** 01 — Plataforma e arquitetura; 37 — Política e resolução de caminhos Windows.

**Type:** design

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0006 — publicação com transação limitada](../../../docs/adr/0006-publicar-exportacao-com-transacao-limitada.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Definir conversão de Unidade e DPI para pixels, incluindo a regra única de arredondamento das dimensões e divisões de Página.
- [ ] Definir ordem de composição, recorte de Frame, transformações da Foto, transparência e comportamento de imagens esticadas de Background e Overlay.
- [ ] Definir `CompositionCore` como módulo puro: entrada imutável, `CompositionPlan` determinístico e nenhuma dependência de I/O, codecs, Cache, PixiJS ou estado mutável; prévia e Exportação devem exercer os mesmos casos dourados.
- [ ] Definir orientação EXIF, perfis e espaço de cor, alfa do PNG e qualidade/compressão de JPEG.
- [ ] Declarar o contrato de entrada exigido do ticket 03 sem depender de sua implementação, permitindo que as duas decisões avancem em paralelo.
- [ ] Definir como o PDF reutilizará a mesma composição visual sem divergência do JPEG/PNG.
- [ ] Definir um `RenderSnapshot` imutável, produzido e validado pelo `ProjectCore`, como única entrada criativa de `MyAlbuns.Imaging.exe`; o Processador não interpreta o documento de Projeto.
- [ ] Definir `ExportPipeline` com `ExportPlanner`, `ExportExecutor` e `Publisher` internos; o pipeline recebe snapshot e opções imutáveis, não salva nem religa Projetos e é reutilizado pela Exportação normal e pelo lote.
- [ ] Depois que `ExportPlanner` enumerar as raízes necessárias, fazer o proprietário congelar seu contexto em `RootBindingPlan`, enviá-lo ao Processador junto do snapshot e reservar a preparação dentro da própria pasta de Destino; publicar somente após renderização e verificação integrais.
- [ ] Abrir e validar cada original necessário durante a tentativa mesmo quando sua raiz já foi resolvida; fatos reutilizados da raiz nunca substituem existência, conteúdo ou identidade do arquivo.
- [ ] Tratar indisponibilidade, permissão, identidade inconclusiva e falta de capacidade atômica como resultados explícitos; nenhum I/O de UNC bloqueia a thread da interface.
- [ ] Definir e documentar o comportamento quando um original muda durante a tentativa, garantindo que a publicação final não misture silenciosamente versões diferentes.
- [ ] Definir no `Publisher` a transação limitada: verificar suporte no uso, promover os arquivos um a um com substituição atômica por arquivo quando suportada e não prometer rollback do conjunto depois que a publicação começar.
- [ ] Se a publicação falhar depois da primeira promoção, encerrar como falha, avisar que o Destino pode conter uma mistura de arquivos anteriores e novos e orientar uma nova Exportação integral; não criar backups integrais nem manifesto permanente.
- [ ] Remover artefatos temporários em sucesso ou falha normal e limpar órfãos aplicáveis somente depois da publicação completa.
- [ ] Definir a numeração depois de `999` sem mudar silenciosamente a convenção `nome-do-projeto_001`.
- [ ] Produzir composições canônicas e resultados esperados que possam validar futuramente o renderizador.
