# Diagramação de Álbuns

Este arquivo fixa a linguagem ubíqua do MyAlbuns. Ele define o significado dos termos do domínio, mas não é fonte de fluxos, algoritmos, detalhes de interface ou critérios de aceite.

## Projeto e estado

**Projeto**:
Documento independente que reúne um Álbum, suas mídias vinculadas, padrões e decisões de diagramação.

**Identidade do Projeto**:
Identificador persistente que distingue um Projeto de todos os demais, mesmo quando possuem o mesmo Nome ou conteúdo inicial.

**Nome do Projeto**:
Nome atribuído ao Projeto e usado como base para sua apresentação e para os nomes de suas saídas.

**Localização do Projeto**:
Caminho do arquivo que contém o estado persistido de um Projeto.

**Instância de arquivo do Projeto**:
Objeto físico que contém o estado persistido de um Projeto em determinado momento. Caminhos diferentes podem alcançar a mesma instância, e uma Cópia externa cria outra instância mesmo quando repete inicialmente a Identidade do Projeto.

**Isolamento de Projeto**:
Propriedade segundo a qual alterações em um Projeto nunca modificam o estado criativo de outro Projeto.

**Sessão do Projeto**:
Estado de trabalho de um Projeto aberto, que pode conter alterações ainda não persistidas.

**Revisão salva**:
Versão do estado criativo registrada pelo último Salvamento do Projeto.

**Salvamento do Projeto**:
Persistência explícita da Sessão do Projeto em sua Localização.
_Evitar_: Salvamento automático

**Salvar como**:
Criação de um novo Projeto independente a partir da Sessão atual, com nova Identidade, Nome ou Localização.

**Cópia de Projeto**:
Projeto independente que começa com o mesmo conteúdo de outro, sem relação de herança ou sincronização posterior.

**Cópia externa**:
Duplicação do arquivo de Projeto realizada fora do MyAlbuns, que passa a existir como Projeto distinto do original.

**Bloqueio de abertura**:
Exclusividade de edição associada à Identidade do Projeto enquanto sua sessão estiver aberta.
_Evitar_: Lock

**Bloqueio órfão**:
Bloqueio de abertura cujo processo proprietário deixou de existir.

**Histórico da sessão**:
Sequência transitória de alterações usada por Undo e Redo durante uma Sessão do Projeto.

**Recuperação de sessão**:
Estado criativo consolidado e temporário, separado do arquivo do Projeto, usado após uma interrupção inesperada. Uma sessão restaurada continua não salva, conhece a Revisão salva de origem e começa com o Histórico da sessão vazio.

## Estrutura do Álbum

**Álbum**:
Sequência ordenada de pelo menos duas Lâminas pertencente a exatamente um Projeto.

**Lâmina**:
Unidade de diagramação formada por um lado esquerdo e um lado direito, dos quais um ou ambos podem representar Páginas.
_Evitar_: Dupla de páginas

**Lado ativo**:
Lado de uma Lâmina que representa uma Página e participa da composição.

**Lado inativo**:
Lado de uma Lâmina que não representa uma Página, não contém conteúdo e não recebe interação própria de Página. Sua representação pode integrar hover, foco e ações da Lâmina.
_Evitar_: Página vazia, página em branco

**Página**:
Superfície de diagramação correspondente a exatamente um Lado ativo.

**Lâmina dupla**:
Lâmina cujos lados esquerdo e direito estão ativos.

**Lâmina de página única**:
Lâmina de extremidade que possui somente um Lado ativo.

**Lâmina inteira**:
Expressão espacial para a superfície completa de uma Lâmina em determinado contexto. Não identifica um tipo estrutural de Lâmina nem o escopo visual chamado Ambos os lados.

**Lâmina inicial**:
Primeira Lâmina do Álbum; quando é de página única, seu Lado ativo é o direito.

**Lâmina interna**:
Lâmina situada entre as extremidades do Álbum e composta por dois Lados ativos.

**Lâmina final**:
Última Lâmina do Álbum; quando é de página única, seu Lado ativo é o esquerdo.

**Papel da Lâmina**:
Classificação de uma Lâmina como inicial, interna ou final segundo sua posição na sequência do Álbum.

**Configuração das extremidades**:
Definição independente de a Lâmina inicial e a Lâmina final serem duplas ou de página única.

**Reorganização da extremidade**:
Conversão de uma Lâmina de extremidade entre os formatos duplo e de página única.

**Numeração de Página**:
Índice sequencial atribuído apenas às Páginas ativas na ordem do Álbum.

## Medidas e acabamento

**Unidade de medida**:
Unidade física única usada pelas medidas de um Projeto, como milímetro, centímetro ou polegada.

**Dimensão da Lâmina**:
Largura e altura físicas da superfície dupla definida para o Projeto.

**Dimensão da Lâmina fechada**:
Representação de entrada usada pela interface de criação quando a Lâmina está
dobrada. Sua largura corresponde à largura da Dimensão da Página, sua altura é a
mesma da Dimensão da Lâmina e sua abertura deriva uma Dimensão da Lâmina com o
dobro da largura informada.

**Dimensão da Página**:
Metade da largura da Dimensão da Lâmina e a mesma altura.

**Resolução do Projeto**:
Densidade, expressa em DPI, usada para converter as medidas físicas do Projeto em saídas rasterizadas.

**Sangria do Projeto**:
Faixa interna à dimensão informada, entre a borda externa e a linha de corte.

**Área de corte**:
Região delimitada internamente pela linha de corte e prevista para permanecer após o acabamento físico.

**Área de segurança**:
Região interna de referência destinada a manter conteúdo importante afastado da linha de corte.

**Margens de acabamento**:
Conjunto formado pela Sangria do Projeto e pela Área de segurança.

**Mudança dimensional segura**:
Alteração de dimensão cuja diferença de proporção permite preservar a intenção visual da composição.

## Composição visual

**Background**:
Base visual de uma composição, formada por uma cor ou por uma Imagem decorativa.
_Evitar_: Fundo

**Overlay**:
Camada decorativa opcional exibida acima de Frames e Fotos, com transparência preservada.
_Evitar_: Moldura

**Imagem decorativa**:
Arquivo vinculado destinado ao uso como Background ou Overlay, e não ao preenchimento de um Frame.

**Ambos os lados**:
Escopo visual no qual uma única aplicação de Background ou Overlay abrange conjuntamente todos os Lados ativos da Lâmina.
_Evitar_: Lâmina inteira, quando o assunto for escopo visual

**Por lado**:
Escopo visual no qual cada Lado ativo possui aplicação independente de Background ou Overlay.

**Padrão visual do Projeto**:
Definição de Background e Overlay usada como origem compartilhada pelas aplicações herdadas.

**Aplicação herdada**:
Aplicação visual que acompanha o Padrão visual do Projeto enquanto não tiver sido substituída localmente.
_Evitar_: default na interface

**Aplicação personalizada**:
Aplicação visual local que deixa de acompanhar o Padrão visual do Projeto.
_Evitar_: custom na interface

**Ausência personalizada**:
Aplicação personalizada que mantém Background ou Overlay deliberadamente ausente, mesmo quando o padrão correspondente existe.

**Restauração do padrão**:
Retorno de uma aplicação ou estilo personalizado à sua origem herdada.

**Foto**:
Arquivo vinculado de conteúdo fotográfico disponível para preencher Frames. Cada colocação possui ajustes próprios sem modificar o arquivo original.

**Frame**:
Região geométrica que mascara e apresenta uma Foto dentro da composição.

**Geometria do Frame**:
Posição, dimensões e orientação espacial de um Frame.

**Estilo do Frame**:
Conjunto de propriedades visuais do Frame, como Borda e Opacidade.

**Borda do Frame**:
Contorno configurável de um Frame, definido por espessura e cor.

**Opacidade do Frame**:
Grau de transparência aplicado a uma ocorrência de Frame.

**Padrão de Frame do Projeto**:
Estilo de Frame compartilhado que serve de origem para Frames herdados.

**Frame herdado**:
Frame cujo estilo acompanha o Padrão de Frame do Projeto.

**Frame personalizado**:
Frame cujo estilo foi definido localmente e não acompanha alterações posteriores do padrão.

**Frame placeholder**:
Frame sem Foto, mantido como espaço reservado na composição.

**Preenchimento do Frame**:
Recorte da Foto que cobre toda a área interna do Frame sem deixar regiões vazadas.

**Zoom de preenchimento**:
Escala mínima calculada para garantir o Preenchimento do Frame.

**Zoom do usuário**:
Acréscimo de escala escolhido pelo usuário sobre o Zoom de preenchimento.

**Pan da Foto**:
Deslocamento da Foto dentro de seu Frame sem alterar a Geometria do Frame.

**Ajustes da Foto**:
Transformações não destrutivas próprias de uma colocação, como Pan, Zoom, espelhamento, preto e branco, giro em passos de 90 graus e Ângulo da Foto.

**Transformação da Foto**:
Parte persistente dos Ajustes da Foto que determina seu enquadramento dentro do Frame, incluindo Pan, Zoom do usuário, espelhamento, giro e Ângulo da Foto. Participa da composição, do Histórico, do Salvamento e da Exportação.

**Ângulo da Foto**:
Rotação fina da Foto, independente do giro em passos de 90 graus.

**Pilha visual**:
Ordem relativa em que Frames são compostos quando suas áreas se sobrepõem.

**Travessia central**:
Condição de um Frame que ocupa áreas dos dois lados da divisão central de uma Lâmina dupla.

**Seleção de Frames**:
Conjunto transitório de Frames que recebe uma ação de edição em comum.

**Caixa delimitadora**:
Retângulo único que envolve uma Seleção de Frames e oferece as alças de redimensionamento do conjunto.

**Caixa de seleção**:
Gesto de arrastar sobre a área vazia de uma superfície para selecionar todos os elementos contidos no retângulo resultante.
_Evitar_: Seleção por arrasto, laço

**Troca de lados**:
Ação que move os Frames inteiramente contidos em uma Página para a posição relativa equivalente da Página oposta, sem espelhar conteúdo nem alterar a Numeração de Página.

## Layouts

**Layout**:
Organização reutilizável de geometrias de Frames para uma quantidade e um Escopo do Layout compatíveis.

**Escopo do Layout**:
Classificação de um Layout como Layout de Lâmina ou Layout de Página.

**Layout de Lâmina**:
Layout cuja organização considera a superfície conjunta da Lâmina e pode incluir Travessia central.

**Layout de Página**:
Layout cuja organização é formada por blocos independentes em cada Página e não admite Travessia central.

**Bloco de Frames**:
Subconjunto de Frames tratado como uma unidade visual dentro de um Layout.

**Compatibilidade de Layout**:
Correspondência entre um Layout e a quantidade de Frames, o formato da Lâmina e o Escopo do Layout exigidos pela composição.

**Layout do sistema**:
Layout disponibilizado pelo Gerador de Layouts e compartilhado pelo aplicativo.

**Gerador de Layouts**:
Componente conceitual responsável por produzir Layouts do sistema; seu algoritmo pertence a uma definição futura.

**Layout personalizado**:
Layout incluído pelo usuário no Catálogo global de Layouts. Sua identidade considera escopo, quantidade, geometria e sequência ordenada dos Frames.

**Catálogo global de Layouts**:
Conjunto de Layouts personalizados disponível aos Projetos do mesmo usuário.

**Layout favorito do Projeto**:
Cópia local de um Layout mantida entre as preferências de um Projeto.

**Organização aplicada**:
Geometria de Frames incorporada à Lâmina depois da aplicação de um Layout, sem vínculo vivo com o Layout de origem.

**Último Layout aplicado**:
Identidade informativa da última organização escolhida para uma Lâmina.

**Mapeamento de Layout**:
Correspondência entre os Frames existentes e as posições ordenadas definidas por um Layout.

**Layout travado**:
Estado da Lâmina que protege posição e dimensões dos Frames de alterações diretas, sem congelar Fotos ou estilos.

## Superfícies de trabalho

**Tela de Boas-vindas**:
Superfície global de entrada do MyAlbuns, separada das Sessões de Projeto.

**Janela do Projeto**:
Superfície de trabalho dedicada a uma Sessão do Projeto.

**Canvas contínuo**:
Área de composição que apresenta as Lâminas do Álbum em uma sequência navegável.

**Lâmina centralizada no Canvas**:
Lâmina mais próxima do centro visual do Canvas e usada como contexto implícito quando um comando não possui alvo explícito.

**Modo normal do Canvas**:
Contexto de trabalho do Canvas contínuo que mantém a sequência de Lâminas navegável e permite interações diretas sem isolar uma única Lâmina.

**Modo de edição da Lâmina**:
Contexto de trabalho dedicado à edição estrutural dos Frames de uma única Lâmina.

**Transformação da visualização**:
Zoom e deslocamento transitórios usados para navegar no Canvas durante o Modo de edição. Não alteram a composição e não participam do Histórico, do Salvamento ou da Exportação.

**Barra da Lâmina**:
Identificação contextual associada a uma Lâmina no Canvas.

**Grade de Lâminas**:
Representação resumida e ordenada das Lâminas do Álbum.

**Painel de Layouts**:
Superfície usada para consultar e aplicar Layouts compatíveis à Lâmina em contexto.

**Painel contextual**:
Superfície única e reutilizável da Janela do Projeto que apresenta as ferramentas do contexto ativo, seja o Álbum, a Lâmina ou um Frame e sua Foto.
_Evitar_: Painel direito

**Painel de imagens**:
Catálogo de Arquivos vinculados do Projeto, separado entre Fotos e Imagens decorativas.

**Filtro de uso**:
Filtro do Painel de imagens que restringe os itens exibidos conforme a mídia esteja ou não referenciada pelo Projeto.

**Tela de Problemas**:
Superfície tabular reutilizável para apresentar entidades problemáticas e ações de resolução.

**Progresso de operação**:
Representação geral do avanço de uma operação, determinada por uma contagem `X/Y` ou indeterminada quando não existe total mensurável.

**Configurações do aplicativo**:
Preferências globais do usuário, externas ao conteúdo e ao Histórico de qualquer Projeto.

**Comando do aplicativo**:
Ação nomeada por uma identidade estável e independente da tecla, do gesto ou do menu que a aciona.

**Integração com Photoshop**:
Integração opcional que abre no Adobe Photoshop o Arquivo vinculado original de uma Foto.

## Arquivos vinculados e Cache

**Arquivo vinculado**:
Arquivo externo referenciado por caminho pelo Projeto e nunca incorporado ao documento.

**Arquivo ausente**:
Arquivo vinculado cuja origem está acessível, mas que não existe no caminho registrado.

**Arquivo indisponível**:
Arquivo vinculado cuja existência não pôde ser confirmada porque sua origem de armazenamento ou o acesso a ela está temporariamente indisponível. Não equivale a Arquivo ausente.

**Religação de arquivo**:
Substituição do caminho registrado de um Arquivo ausente por um caminho válido dentro de um único Projeto.

**Monitor de Arquivos vinculados**:
Observador de mudanças nos Arquivos vinculados, usado para invalidar representações derivadas sem alterar o original.

**Cache de mídia**:
Armazenamento descartável, separado por Identidade do Projeto, com uma representação visual reduzida e metadados derivados por mídia.

**Contexto de resolução de caminhos**:
Estado técnico e temporário pertencente ao componente que conduz uma tentativa de Importação, geração de Cache, Religação, Exportação ou lote. Ele reutiliza bindings durante o planejamento e, quando o trabalho atravessa processos, é congelado em um plano imutável compartilhado somente pelos participantes. Contexto e plano não persistem existência ou identidade de arquivos individuais e são descartados ao terminar. Não é Cache de mídia nem mapa funcional de Religação.

**Uso do Cache**:
Medida do espaço ocupado pelas representações descartáveis mantidas pelo aplicativo.

**Aviso de espaço**:
Aviso associado à pressão real de armazenamento do volume que hospeda o Cache.

**Liberar espaço**:
Remoção do Cache pertencente a Projetos fechados, preservando Projetos ativos e todos os Arquivos vinculados.

**Limpeza total do Cache**:
Remoção de todo o Cache quando não existem Projetos ou processos ativos, ou limpeza agendada para a próxima inicialização segura.

## Exportação

**Exportação**:
Produção de saídas finais a partir dos Arquivos vinculados originais e do estado criativo selecionado.

**Exportação normal**:
Exportação iniciada a partir de uma Sessão do Projeto e capaz de usar seu estado visível ainda não salvo.

**Por lâmina**:
Modo de Exportação que produz uma unidade para cada Lâmina, mantendo juntas suas Páginas ativas.
_Evitar_: Lâmina completa

**Por página**:
Modo de Exportação que produz uma unidade separada para cada Página ativa.

**Exportação integral**:
Exportação cujo conjunto selecionado corresponde ao Álbum inteiro.

**Exportação parcial**:
Exportação de um Intervalo de exportação que preserva os índices originais das unidades selecionadas.

**Intervalo de exportação**:
Trecho contínuo da sequência de Lâminas escolhido para uma Exportação parcial.

**Destino da Exportação**:
Pasta que recebe as saídas publicadas de um Projeto.

**Formato de exportação**:
Codificação da saída final, entre JPEG, PNG e PDF.

**Unidade de Exportação**:
Superfície final correspondente a uma Lâmina ou Página ativa, conforme o modo escolhido, que se torna uma imagem ou uma página do PDF.

**Qualidade JPEG**:
Nível de compressão usado por uma Exportação JPEG; o lote usa qualidade máxima.

**Nome de imagem exportada**:
Nome JPEG ou PNG no formato `{Nome do Projeto}_{índice decimal com largura mínima de três dígitos}`, com índice de Lâmina no modo Por lâmina ou de Página no modo Por página.
_Evitar_: Sufixos `lamina` e `pagina`

**Namespace JPEG/PNG da Exportação**:
Família compartilhada de nomes `{Nome do Projeto}_{índice decimal com largura mínima de três dígitos}` usada pelos modos Por lâmina e Por página em um mesmo Destino da Exportação. O nome isolado não identifica em qual modo uma saída foi produzida.

**Conflito de exportação**:
Existência, no Destino da Exportação, de um nome final que também pertence ao conjunto preparado.

**Saída órfã de exportação**:
Arquivo pertencente ao Namespace JPEG/PNG da Exportação que não integra o novo conjunto de uma Exportação integral.

**Exportação em PDF**:
Exportação consolidada em um documento multipágina chamado `{Nome do Projeto}.pdf`.

**Preparação da Exportação**:
Produção e verificação de todas as saídas selecionadas em uma pasta temporária reservada dentro do próprio Destino da Exportação, antes de sua Publicação.

**Publicação da Exportação**:
Promoção do conjunto preparado aos nomes finais no Destino da Exportação, seguida da remoção aplicável de Saídas órfãs.

**Transação limitada de Publicação**:
Contrato que só considera a Exportação bem-sucedida depois de todo o conjunto promovido, com atomicidade garantida por arquivo e não pelo conjunto.

**Bloqueio global de Exportação normal**:
Exclusividade que permite somente uma Exportação normal ativa entre os Projetos abertos.

**Problema de Exportação**:
Condição que impede preparar uma saída selecionada, como um Frame placeholder, um Arquivo ausente necessário ou um Arquivo indisponível cujo original não pôde ser aberto.

## Operações em lote

**Projeto modelo**:
Sessão do Projeto usada como origem para criar Projetos independentes em uma Geração de Projetos em lote.

**Geração de Projetos em lote**:
Operação que cria Projetos independentes a partir de Pastas de Fotos e de um Projeto modelo.

**Pasta de Fotos**:
Pasta de origem que contém imagens e representa um Projeto a ser gerado.

**Espelhamento da estrutura de pastas**:
Preservação, no destino, da hierarquia relativa que organiza as Pastas de Fotos de origem.

**Caminho de Projeto gerado**:
Localização calculada pela combinação do espelho da pasta-pai e do nome da Pasta de Fotos.

**Exportação em lote**:
Exportação integral e exclusiva de vários Projetos encontrados em uma árvore de pastas, a partir do estado persistido de cada um.

**Modo de lote exclusivo**:
Estado global no qual a Exportação em lote possui uso exclusivo do aplicativo.

**Item de lote**:
Projeto individual avaliado e processado por uma Exportação em lote.

**Pré-validação do lote**:
Validação dos estados persistidos que serão usados para criar os snapshots dos Itens de lote.

**Resolução temporária de arquivo para lote**:
Associação transitória de um Arquivo ausente a um original válido, aplicável individualmente a um Item de lote ou por uma ação global. A resolução global só aceita uma correspondência exata e única dentro da pasta de mesmo nome do Projeto; nenhuma resolução é salva no Projeto.

**Checkpoint de lote**:
Registro mínimo do resultado terminal de cada Item de lote e do item que estava pendente quando ocorreu uma interrupção.

**Recuperação de lote**:
Retomada explícita de um lote interrompido a partir de seu Checkpoint; um Item interrompido é refeito por inteiro.

**Destino alternativo do lote**:
Raiz opcional que recebe as saídas dos Itens de lote preservando sua hierarquia relativa.

**Resultado de lote**:
Resumo dos Itens concluídos, ignorados ou com falha em uma operação em lote.
