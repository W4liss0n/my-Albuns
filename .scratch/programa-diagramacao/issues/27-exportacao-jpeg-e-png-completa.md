# 27 — Exportação JPEG e PNG completa

**What to build:** entregar a tela completa de Exportação de imagens, capaz de renderizar o Álbum inteiro ou um Intervalo como `Por lâmina` ou `Por página`, usando o estado visível atual e os Arquivos originais.

**Blocked by:** 04 — Renderizador final; 26 — Conversão de extremidades.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [Exportação normal](../../../docs/design/0004-exportacao-normal.md); [Tela de Problemas](../../../docs/design/0005-tela-de-problemas.md); [ADR 0006 — publicação com transação limitada](../../../docs/adr/0006-publicar-exportacao-com-transacao-limitada.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] A tela permite escolher JPEG ou PNG, Álbum inteiro ou Intervalo contínuo e modo `Por lâmina` ou `Por página`.
- [ ] Organizar Escopo, Modo, Formato e Destino em um modal único, com resumo calculado da quantidade de arquivos e sem permitir alterar dimensões ou DPI.
- [ ] Exibir slider de qualidade somente para JPEG na Exportação normal; PNG não apresenta esse controle, e o ajuste não altera Projeto, Salvamento ou Undo/Redo.
- [ ] Inicializar o slider no máximo em cada abertura e fazer dois cliques restaurarem esse valor, sem reutilizar a redução em uma Exportação futura.
- [ ] Manter a qualidade escolhida restrita à operação normal atual; ela não redefine a regra do lote.
- [ ] `Exportar Lâmina` no menu contextual abre a tela com início e fim preenchidos pela Lâmina selecionada.
- [ ] A Exportação normal usa o estado visível atual, inclusive mudanças não salvas, sem executar `Salvar`.
- [ ] Adquirir `NormalExport` no `OperationGate`: somente uma Exportação normal pode estar ativa, as demais ações ficam indisponíveis e não criam fila, enquanto edição e Salvamento de outras janelas continuam disponíveis.
- [ ] Antes de reservar o Processador de Imagens da Sessão, obter do `CacheEngine` a pausa do trabalho de Cache do Projeto exportado; retomar a fila em sucesso, falha ou cancelamento.
- [ ] O progresso usa um modal pertencente somente à janela do Projeto exportado, bloqueando esse Projeto sem aparecer ou bloquear os demais.
- [ ] A validação considera somente a seleção e bloqueia placeholders, Arquivos ausentes ou Arquivos indisponíveis realmente necessários, apresentando todos os motivos antes de renderizar.
- [ ] Apresentar bloqueios num diálogo separado em tabela com Projeto, Problema e Ações; placeholder abre o Projeto, Arquivo ausente permite escolher a pasta de Fotos para Religar e Arquivo indisponível oferece `Tentar novamente`.
- [ ] A Religação individual procura recursivamente na pasta escolhida pelo nome e extensão exatos do Arquivo original e resolve automaticamente somente uma correspondência única; ausência ou ambiguidade permanece como problema.
- [ ] Uma Religação individual bem-sucedida altera a sessão aberta, entra no Undo/Redo e fica não salva, mas pode alimentar imediatamente a Exportação normal.
- [ ] Nunca iniciar automaticamente ao resolver problemas; exigir `Continuar Exportação`, e cancelar a tentativa sem desfazer relinks já realizados na sessão.
- [ ] Cada saída usa toda a superfície ativa e inclui a Sangria interna, sem guias; lados inativos não geram arquivo, lacuna ou número.
- [ ] No modo `Por página`, Frames com Travessia central são recortados exatamente na divisão.
- [ ] Nomes seguem `{nome-do-projeto}_{índice com três dígitos}`; intervalos preservam posições originais, e o comportamento após `999` segue o ticket 04.
- [ ] O destino padrão é uma pasta com o Nome do Projeto ao lado do arquivo; o usuário pode escolher outro caminho local, UNC, unidade mapeada ou caminho longo aceito pelo contrato.
- [ ] Todos os conflitos são pré-calculados em conjunto e o modal oferece `Sobrescrever todos` ou `Cancelar`, sem renomear ou sobrescrever silenciosamente.
- [ ] O `ProjectCore` valida o estado visível e produz um `RenderSnapshot` imutável; o mesmo `ExportPipeline` usado pelo lote planeja, executa e publica sem salvar ou religar o Projeto. `MyAlbuns.Imaging.exe` lê os originais, nunca o Cache, e não interpreta o documento bruto.
- [ ] Depois do planejamento enumerar as raízes, o proprietário congela seu contexto em `RootBindingPlan` e o envia ao Processador junto do snapshot; todos usam os mesmos bindings, ainda abrem individualmente cada original e não bloqueiam a thread da interface com rede.
- [ ] A renderização apresenta progresso, cancelamento seguro e resumo de falhas.
- [ ] Após sucesso integral, fechar o progresso com confirmação curta; em falha, usar a Tela de Problemas para o resultado, sem transformar o progresso em relatório.
- [ ] Renderizar e verificar toda a seleção em preparação temporária reservada dentro da própria pasta de Destino antes de publicar; a saída nunca é anunciada como concluída enquanto a publicação não terminar.
- [ ] Fazer o `Publisher` verificar o suporte e promover cada arquivo com substituição atômica quando disponível, sem rollback do conjunto. Falha depois da primeira promoção informa que o Destino pode misturar saídas anteriores e novas e recomenda uma nova Exportação integral.
- [ ] Remover preparações temporárias quando for seguro; órfãos só podem ser removidos depois de uma publicação completa bem-sucedida e nunca são apagados após falha.
- [ ] Se o Processador de Imagens cair, a Exportação falha sem repetição automática, concessão, progresso, cancelamento e qualquer pausa de Cache são liberados, o Processador é reiniciado e o modal oferece `Tentar novamente` ou `Fechar`.
- [ ] Testes verificam dimensões físicas/DPI, ordem, transparência, pilha visual, nomes, estado não salvo e equivalência com o editor.
