# 01 — Plataforma e arquitetura

**What to build:** validar e estabelecer uma fundação técnica executável para o aplicativo desktop, capaz de sustentar edição visual responsiva, acesso a arquivos locais, persistência explícita e renderização final independente das prévias.

**Blocked by:** None — can start immediately.

**Type:** spike

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0005 — Tauri, React e Rust](../../../docs/adr/0005-adotar-tauri-react-rust.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Construir um único cenário representativo em Windows 10/11 x64 com Tauri 2, React/TypeScript, Rust e PixiJS sobre WebGL2: Álbum longo, Fotos grandes, Frames, máscara, Pan, Zoom, Overlay, seleção, Undo/Redo, Cache e Exportação por snapshot.
- [ ] Executar o mesmo cenário em duas alternativas comparáveis: (A) um host de Projeto independente por Projeto aberto; (B) um único host de Projeto multiwindow, ou equivalente, com contextos e Processadores de Imagens isolados por Projeto.
- [ ] Manter `MyAlbuns.exe` como candidato a processo leve de Boas-vindas e operações globais nas duas alternativas; a topologia final não é considerada decidida antes das medições.
- [ ] Validar `ProjectCore` como seam externo pequeno usado pela Janela e pelo lote, mantendo `ProjectDomain`, `ProjectSession` e `ProjectStore` como subdivisões inicialmente internas; provar que existe uma única `ProjectSession` mutável por Projeto aberto.
- [ ] Provar as duas entradas do `ProjectCore` com garantias diferentes: abrir sessão editável e carregar revisão persistida. Demonstrar que a segunda não instancia `ProjectSession`, não oferece comando, Salvamento ou Undo/Redo, e que o `BatchRunner` processa um lote inteiro sem que o processo hospedeiro passe a possuir estado criativo mutável nem consiga gravar no arquivo do usuário.
- [ ] Validar um `CompositionCore` puro e determinístico reutilizado por prévia e Exportação, distinguindo `MediaTransform` persistente de `ViewportTransform` transitório e ausente do `RenderSnapshot`.
- [ ] Fazer `MyAlbuns.Imaging.exe` aceitar somente snapshots imutáveis já validados; ele não pode abrir ou interpretar por conta própria o documento persistido do Projeto.
- [ ] Manter hover, seleção e geometria transitória do gesto no frontend e enviar ao núcleo somente a intenção consolidada ao final; React/TypeScript não duplica regras canônicas do domínio.
- [ ] Aplicar o critério do ADR 0005 para repartir as regras: o que sobrevive ao Salvamento ou participa do Undo/Redo pertence ao núcleo. Exercitar ao menos uma regra determinística que a SPEC descreve dentro de um gesto mas cujo efeito é documental — ordenação do placeholder mais à esquerda, Frame atingido em sobreposição ou deslocamento de colagem — provando que o TypeScript envia entradas e recebe a decisão, sem calculá-la.
- [ ] Usar uma única representação visual reduzida por Foto ou Decorativo como baseline do Cache. Medir essa solução antes de introduzir tiles, níveis progressivos ou previews persistidos de Lâmina.
- [ ] Preservar todas as Lâminas no modelo lógico, mas materializar cena detalhada e texturas somente para o viewport e uma margem de pré-carregamento; registrar a política de descarte e testar navegação em Álbuns longos.
- [ ] Demonstrar perda e recuperação de contexto WebGL2, limites de textura e pressão de memória gráfica. Se não houver aceleração WebGL2 utilizável, manter Boas-vindas, Configurações e diagnóstico acessíveis e bloquear o editor com orientação clara.
- [ ] Abrir ao menos dois Projetos simultaneamente e medir, em cada alternativa, tempo de abertura, latência do Canvas e de Pan/Zoom, memória de processo, memória gráfica, quantidade de processos, vazão do Cache e tempo de Exportação.
- [ ] Forçar a queda de `MyAlbuns.exe`, do host de Projeto e do Processador de Imagens e comparar isolamento, continuidade de edição/Salvamento local, recuperação, complexidade de IPC e qualidade dos logs. Não implementar eleição ou reinício automático do processo principal como requisito do MVP.
- [ ] Se a topologia permitir que Janelas sobrevivam à queda de `MyAlbuns.exe`, demonstrar edição e Salvamento locais enquanto operações globais ficam indisponíveis até reinício explícito protegido por instância única.
- [ ] Demonstrar reinício seguro do Processador durante Cache e falha controlada durante Exportação, sem corromper Projeto nem anunciar uma publicação incompleta como sucesso.
- [ ] Restringir o frontend com capabilities, permissions e scopes mínimos; não expor acesso genérico ao sistema de arquivos nem permissão genérica de shell.
- [ ] Provar a interface proposta para `AppPaths`, `RootBindingPlan` e `OperationPathContext` com caminhos locais, UNC, unidade mapeada, VerbatimDisk e VerbatimUNC, mantendo toda operação de rede fora da thread da interface.
- [ ] Nas duas topologias, transmitir pela IPC o mesmo `RootBindingPlan` ao host e ao Processador, impedir resolução independente de raízes já capturadas e criar um plano novo em outra tentativa.
- [ ] Comparar as bibliotecas candidatas atrás dessa interface, incluindo descoberta das Known Folders sem alterar a árvore `%APPDATA%\MyAlbuns` e `%LOCALAPPDATA%\MyAlbuns`, igualdade física por handles e limites de caminhos longos; registrar evidências sem tornar uma crate contrato do produto.
- [ ] Demonstrar que uma unidade mapeada e seu alias UNC do mesmo Projeto focalizam uma única sessão, que resultado de identidade inconclusivo falha de forma fechada e que o bloqueio do arquivo continua sendo a proteção final.
- [ ] Demonstrar uma Exportação para destino local e outra para destino UNC com staging dentro da própria pasta de Destino, originais abertos individualmente e falha recuperável se o binding operacional capturado não puder mais ser usado.
- [ ] Validar `CacheEngine`, `ExportPipeline` e `OperationGate` como responsabilidades separadas; nas duas topologias, provar exclusividade realmente global entre duas Janelas e liberação de concessões, pausa, cancelamento e progresso em sucesso, falha, cancelamento ou queda do proprietário, sem criar um coordenador universal.
- [ ] Provar o `OperationLease` como único caminho de reserva: Exportação normal e cada item do lote adquirem concessão, pausa do Cache e Processador pelo mesmo lease e devolvem os três juntos. Injetar falha em cada ponto — antes da preparação, entre duas promoções e por queda do proprietário — e demonstrar que nenhuma reserva vaza e que uma nova Exportação inicia normalmente logo depois.
- [ ] Demonstrar que `OperationGate` e Bloqueio de abertura permanecem mecanismos distintos: a concessão morre com o processo, enquanto a trava do arquivo sobrevive e só é recuperada quando o processo proprietário deixa de existir.
- [ ] Registrar a opção tecnicamente viável para serializar e restaurar caminhos Windows sem conversão com perda; a escolha final pertence ao formato do Projeto no ticket 02.
- [ ] Validar WebView2 Evergreen, diálogo nativo de arquivo, instalador `win-x64` e um teste ponta a ponta em máquina limpa.
- [ ] Registrar hardware, massas de teste, medições brutas, falhas e custo de implementação de cada alternativa. Congelar os critérios de comparação antes da execução final.
- [ ] Encerrar o spike com recomendação explícita de topologia, riscos, evidências reproduzíveis e atualização do ADR 0005; WPF/C# só é reavaliado se os gates acordados falharem.

## Comments

- 2026-07-28: a revisão externa manteve Tauri/React/Rust como direção principal, mas reabriu a topologia de processos para comparação objetiva neste spike.
- 2026-07-28: a pesquisa técnica de caminhos está em `docs/research/0004-caminhos-windows-e-unc.md`; ela informa o spike, mas não substitui as fontes normativas deste ticket.
- 2026-07-28: uma revisão de arquitetura acrescentou quatro critérios. Três decisões novas precisavam de gate próprio — as duas entradas do `ProjectCore`, o `OperationLease` e o critério de reparte entre núcleo e interface — e o critério existente sobre `ProjectSession` não as cobria, porque fala em Projeto "aberto" e o lote não abre nenhum.
