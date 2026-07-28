# 30 — Geração de Projetos em lote

**What to build:** usar o Projeto aberto como modelo para gerar recursivamente Projetos completos e independentes a partir de uma árvore de pastas com imagens, preservando a hierarquia relativa no destino.

**Blocked by:** 13 — Bloqueio de abertura; 19 — Painel de imagens; 21 — Layout travado; 23 — Layouts favoritos do Projeto.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [configuração da Geração em lote](../../../docs/design/0008-configuracao-da-geracao-em-lote.md); [Tela de Problemas](../../../docs/design/0005-tela-de-problemas.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md).

- [ ] A Janela do Projeto modelo abre uma janela dedicada com Nome do modelo somente leitura, pasta de origem, pasta de destino, contagem de pastas geradoras e `Cancelar`/`Verificar e gerar`.
- [ ] `Verificar e gerar` executa toda a descoberta e a pré-validação antes de criar ou sobrescrever qualquer arquivo e abre a Tela de Problemas quando necessário.
- [ ] O diálogo permite escolher origem e destino entre os caminhos Windows aceitos, mostra as regras da operação e rejeita destino igual à origem ou contido nela, inclusive quando unidade mapeada e UNC são aliases.
- [ ] Cada tentativa reutiliza um `OperationPathContext` para as raízes de origem e Destino e o congela em `RootBindingPlan` antes de distribuir trabalho; qualquer processo participante usa esse plano, enquanto arquivos e pastas continuam sendo acessados e validados individualmente fora da thread da interface.
- [ ] O estado visível integral do Projeto modelo, inclusive mudanças não salvas, é copiado sem salvar ou modificar o modelo.
- [ ] Cada pasta que contenha diretamente ao menos uma imagem importável gera um Projeto com seu Nome; a busca continua recursivamente nas subpastas.
- [ ] A hierarquia relativa é recriada por componentes validados, sem duplicar a pasta geradora nem permitir escape do Destino: `origem/Turma 1/001` produz o Projeto `001` em `destino/Turma 1`.
- [ ] Cada Projeto recebe nova Identidade e copia Lâminas, composições, Frames, Fotos existentes, padrões, customizações, travamentos, favoritos e vínculos do modelo.
- [ ] As novas imagens da pasta entram vinculadas somente na aba `Fotos` do Painel e não são colocadas automaticamente nas Lâminas.
- [ ] Conflitos são pré-calculados e aparecem na Tela de Problemas, uma linha por destino existente, com `Sobrescrever`/`Ignorar` por linha e `Sobrescrever todos`/`Ignorar todos` como ações globais.
- [ ] Projeto de destino aberto nunca é incluído em sobrescrita individual ou global; `Sobrescrever` fica indisponível, e o item só pode ser ignorado enquanto permanecer aberto.
- [ ] Habilitar `Continuar Geração` somente depois de todos os conflitos receberem uma decisão e nunca iniciar automaticamente ao resolver a última linha.
- [ ] Uma falha não interrompe os demais itens, e progresso/resumo separam sucessos, ignorados e falhas com seus motivos.
- [ ] Origem ou Destino indisponível aparece como problema recuperável e não é convertido em pasta vazia, arquivo ausente ou sucesso parcial.
- [ ] Testes usam árvores reais, pastas geradoras aninhadas, conflitos, arquivo inválido, modelo não salvo e isolamento entre Projetos.
