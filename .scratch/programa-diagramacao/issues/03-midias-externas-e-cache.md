# 03 — Mídias externas e Cache

**What to build:** decidir como Fotos e Decorativos vinculados serão identificados, decodificados e visualizados com desempenho, mantendo os arquivos originais como única fonte autorizada para operações finais.

**Blocked by:** 01 — Plataforma e arquitetura; 02 — Documento de Projeto e identidade.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0001 — vincular Arquivos externos](../../../docs/adr/0001-vincular-arquivos-externos.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Registrar formatos, bibliotecas e limites de decodificação para JPG/JPEG, PNG e TIFF/TIF.
- [ ] Definir normalização de orientação EXIF e o tratamento explícito de TIFF multipágina.
- [ ] Definir identidade de um item importado, duplicação entre abas, substituição no mesmo caminho e detecção separada de Arquivo ausente e Arquivo indisponível.
- [ ] Manter `MediaRef` persistente no Projeto; fazer `MediaResolver` inspecionar arquivos e produzir observações ou propostas imutáveis de Importação e Religação, `MediaRuntime` registrar os estados observados e `MediaMonitor` apenas sinalizar mudanças que exigem nova inspeção.
- [ ] Manter o Cache por Projeto em `%LOCALAPPDATA%\MyAlbuns\Cache\{project-id}`, usando a Identidade persistente e nunca o Nome ou o caminho do arquivo.
- [ ] Implementar o resultado do spike. O baseline a validar persiste apenas `metadata.json` e uma representação visual reduzida por Foto ou Decorativo em `Media`, sem tiles ou previews de Lâmina em disco e com miniaturas da Grade somente em memória; qualquer revisão exige evidência registrada no ticket 01.
- [ ] Manter no índice descartável schema, versão das regras da representação, Identidade do Projeto, último uso e, por mídia, geração publicada, dimensões, formato, orientação EXIF, tamanho, datas do arquivo, quantidade de páginas aplicável, perfil de cor básico e fingerprint.
- [ ] Manter no Projeto a identidade da mídia, o caminho original, sua categoria e decisões do usuário; metadados em Cache nunca substituem essas informações canônicas.
- [ ] Criar um `OperationPathContext` proprietário por tentativa de Importação ou Cache e congelá-lo em `RootBindingPlan` antes de entregar trabalho ao Processador; não persistir aliases, existência, metadados ou identidade física como cache de caminhos.
- [ ] Tornar `CacheEngine` o proprietário lógico de jobs, índice, artefatos, invalidação, pausa e manutenção; fora de manutenção, usar o `MyAlbuns.Imaging.exe` somente como adaptador escritor da pasta e dar a cada job uma geração e um `.tmp` próprios.
- [ ] Antes de publicar, revalidar pedido, fingerprint e variante; descartar jobs obsoletos, promover o artefato imutável primeiro e substituir por último o índice que referencia a mesma geração.
- [ ] Após commit ou reinício, remover temporários, artefatos antigos sem consumidores e gerações não referenciadas sem comprometer a geração publicada anterior.
- [ ] Fazer o Monitor apenas sinalizar; depois de inspeção autoritativa confirmar divergência, reaparecimento/Religação, schema incompatível, corrupção ou falha de validação, invalidar somente a mídia afetada. Pan, Zoom, Frame e Layout não invalidam a representação da fonte.
- [ ] Aceitar na primeira versão o risco raro de uma alteração feita com o aplicativo fechado preservar exatamente tamanho e data de alteração; não calcular hash completo de todos os originais a cada abertura.
- [ ] Se o original estiver ausente, permitir manter a última representação e os últimos metadados apenas como contexto visual, preservando o estado ausente e o bloqueio de toda Exportação que dependa dele.
- [ ] Se a origem estiver indisponível, preservar vínculo e última representação com estado próprio e `Tentar novamente`; não oferecer Religação nem confirmar ausência até a raiz voltar a ser acessível.
- [ ] Definir o modo de mídia do Processador de Imagens isolado associado ao host de cada Projeto, seus limites de memória e as prioridades entre Lâmina ativa, Painel visível, prefetch, reconstrução e limpeza.
- [ ] Pedidos de preview obsoletos devem ser canceláveis e pedidos equivalentes agrupados sem bloquear a fila de comandos do Projeto.
- [ ] Ao iniciar Exportação, o trabalho de Cache concorrente é pausado ou cancelado com segurança; o Processador responsável pela saída final recebe exclusividade e a fila descartável é retomada depois.
- [ ] Se o Processador de Imagens cair durante o Cache, reiniciá-lo automaticamente, descartar o item incompleto e reconstruir somente os pedidos ainda relevantes a partir do estado canônico e dos originais; falhas repetidas suspendem o Cache e geram aviso, sem bloquear edição ou Salvamento.
- [ ] Garantir que o Cache possa ser apagado sem perder conteúdo do Projeto e nunca autorize Exportação quando o original estiver ausente.
- [ ] Manter o namespace de Cache em `%LOCALAPPDATA%\MyAlbuns` mesmo quando Projeto ou mídia estiver em UNC ou unidade mapeada.
- [ ] Expor ao ticket 32 o total ocupado, o volume liberável e operações de serviço distintas para remover Cache de Projetos fechados ou todo o Cache, sempre preservando Projetos, vínculos e originais.
- [ ] Fazer `MyAlbuns.exe` tentar reservar atomicamente cada namespace antes de `Liberar espaço`; conceder a reserva somente sem proprietário ativo, impedir novo proprietário durante a remoção e preservar a pasta se a reserva falhar.
- [ ] Iniciar a limpeza total somente antes de abrir Projetos ou quando não houver Projeto nem Processador ativo; enquanto ela estiver em execução, impedir a abertura de Projetos e o início de Processadores ou Exportações.
- [ ] Se houver contexto ativo, recusar a limpeza total ao vivo e permitir agendá-la para a próxima inicialização; a limpeza cotidiana continua restrita a namespaces de Projetos fechados.
- [ ] Definir a religação de arquivos por ocorrência, sem alterar outro Projeto ou outra referência que use o mesmo caminho.
- [ ] Usar `docs/design/0010-armazenamento-local-e-cache.md` como contrato técnico e manter sua matriz de cenários coberta por testes.

## Comments

- 2026-07-28: o desenho inicial foi deliberadamente reduzido a uma representação por mídia e um índice descartável. Formato, resolução e eventual adoção de tiles dependem do spike e não devem criar novas categorias de Cache sem medição.
