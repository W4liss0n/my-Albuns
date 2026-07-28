---
status: ready-for-agent
document: product-spec
implementation-readiness: decision-tickets-required
---

# Programa de Diagramação de Álbuns

## Problem Statement

Diagramadores de álbuns precisam organizar muitas Fotos em superfícies físicas de impressão sem perder controle sobre enquadramento, composição, acabamento e identidade visual. Ferramentas excessivamente livres tornam tarefas repetitivas lentas; ferramentas rígidas demais impedem ajustes manuais e criam dependência de modelos fixos.

O trabalho também precisa continuar previsível fora da composição. Um Projeto copiado deve ser realmente independente, arquivos de mídia grandes não devem ser duplicados, alterações não salvas não podem ser persistidas silenciosamente e a Exportação deve sempre usar os originais. Em operações com dezenas ou centenas de álbuns, geração e Exportação em lote precisam preservar hierarquias, isolar falhas e evitar substituições inesperadas.

O produto deve equilibrar quatro necessidades:

- automatizar a organização inicial por Layouts sem manter vínculos ocultos;
- permitir edição não destrutiva e controle manual quando necessário;
- conservar regras físicas claras para Lâminas, Páginas, Sangria e segurança;
- produzir saídas finais confiáveis, inclusive em lote, sem misturar o estado de Projetos diferentes.

## Solution

Construir um aplicativo de diagramação no qual cada Projeto represente exatamente um Álbum composto por Lâminas ordenadas. Cada Lâmina possui lados esquerdo e direito; somente lados ativos representam Páginas. As extremidades podem ser duplas ou de página única, permitindo álbuns que iniciem e terminem com uma única Página sem criar áreas falsas ou interativas.

A composição terá uma Pilha visual fixa: Background, Frames com Fotos e Overlay. Background, Overlay e estilo de Frame poderão herdar padrões do Projeto ou assumir personalizações locais. Fotos permanecerão vinculadas aos arquivos originais e serão editadas de maneira não destrutiva dentro de Frames retangulares.

Layouts organizarão somente posição e dimensão dos Frames. Aplicar um Layout copiará sua geometria para o Projeto; o usuário poderá continuar editando livremente ou travar a quantidade, a posição e o tamanho dos Frames. Layouts do sistema serão gerados, Layouts personalizados poderão integrar um catálogo global e favoritos serão cópias estáveis armazenadas no Projeto.

O Painel de imagens separará Fotos de Decorativos, permitirá importar, ordenar, filtrar e remover referências com consequências explícitas. Salvamento será sempre manual, Undo e Redo serão essenciais, e cada Projeto possuirá identidade própria para permitir cópias verdadeiramente independentes.

A saída final será uma Exportação JPEG, PNG ou PDF, `Por lâmina` ou `Por página`. A Exportação normal poderá abranger todo o Álbum ou um intervalo contínuo; a Exportação em lote sempre abrangerá Álbuns inteiros. Geração de Projetos em lote criará cópias independentes a partir de um Projeto modelo e preservará a árvore de pastas de origem.

## User Stories

### Entrada e janelas

1. Como pessoa diagramadora, quero encontrar uma Tela de Boas-vindas ao iniciar o aplicativo sem um arquivo, para criar, abrir ou localizar um trabalho recente.
1. Como pessoa diagramadora, quero abrir diretamente um arquivo de Projeto pelo sistema operacional, para chegar à sua Janela do Projeto sem uma etapa intermediária obrigatória.
1. Como pessoa diagramadora, quero que a Tela de Boas-vindas permaneça separada do editor, para que cada Projeto continue em sua própria janela e sessão isolada.
1. Como pessoa diagramadora, quero que a Tela de Boas-vindas seja ocultada enquanto trabalho e reapareça ao fechar o último Projeto, para evitar uma janela desnecessária sem perder o ponto de entrada.
1. Como pessoa diagramadora, quero acessar novamente a Tela de Boas-vindas a partir de qualquer Projeto, para criar ou abrir outro trabalho.
1. Como pessoa diagramadora, quero que iniciar novamente o aplicativo reutilize a instância existente, para não receber duas Telas de Boas-vindas ou operações globais concorrentes.
1. Como pessoa diagramadora, quero abrir vários arquivos pelo Explorador e receber uma Janela separada para cada Projeto independente, para iniciar rapidamente meus trabalhos.
1. Como pessoa diagramadora, quero que um Projeto já aberto seja apenas focalizado, para não criar duas edições da mesma Identidade.
1. Como pessoa diagramadora, quero iniciar a Geração de Projetos em lote a partir da Janela do Projeto usado como modelo, para deixar clara a origem da operação.

### Configurações do aplicativo e integrações

1. Como pessoa diagramadora, quero encontrar Configurações organizadas em abas, para separar áreas globais sem misturá-las com o Projeto.
1. Como pessoa diagramadora, quero abrir as mesmas Configurações pela Boas-vindas ou por qualquer Projeto, para não interromper meu fluxo.
1. Como pessoa diagramadora, quero que uma segunda solicitação apenas focalize a janela existente, para não editar preferências globais em duas telas concorrentes.
1. Como pessoa diagramadora, quero que as mudanças se apliquem a todas as Janelas abertas sem reiniciar, para perceber imediatamente a personalização.
1. Como pessoa diagramadora, quero consultar o uso do Cache e as ações de limpeza na aba `Desempenho`, para controlar o espaço local em um único lugar.
1. Como pessoa diagramadora, quero conhecer o total ocupado e quanto pode ser liberado dos Projetos fechados, para compreender o impacto antes da operação.
1. Como pessoa diagramadora, quero liberar somente o Cache de Projetos fechados, para preservar a fluidez dos trabalhos atualmente abertos.
1. Como pessoa diagramadora, quero limpar todo o Cache imediatamente quando não houver Projeto ativo ou agendar a limpeza para a próxima inicialização, para não apagar representações em uso.
1. Como pessoa diagramadora, quero saber que meus Projetos, Fotos, vínculos e originais não serão apagados, para confirmar a limpeza com segurança.
1. Como pessoa diagramadora, quero receber um aviso quando o espaço livre do disco estiver baixo, para agir por uma condição real sem alertas progressivos arbitrários.
1. Como pessoa diagramadora, quero que o programa detecte uma instalação do Photoshop, para saber se a integração está disponível.
1. Como pessoa diagramadora, quero que a versão mais recente seja escolhida inicialmente, para usar a instalação provável sem configuração manual.
1. Como pessoa diagramadora, quero escolher outra instalação detectada ou localizar seu executável, para controlar qual Photoshop será aberto.
1. Como pessoa diagramadora, quero abrir uma Foto do MyAlbuns diretamente no Photoshop, para continuar um tratamento externo.
1. Como pessoa diagramadora, quero que o Photoshop receba sempre o original, para não transformar ajustes não destrutivos do MyAlbuns em alterações incorporadas.
1. Como pessoa diagramadora, quero usar `Ctrl + E` para abrir rapidamente a Foto contextual no Photoshop.
1. Como pessoa diagramadora, quero que uma seleção múltipla não abra várias Fotos no Photoshop, para evitar uma ação externa em massa por engano.
1. Como pessoa diagramadora, quero que alterações externas sejam detectadas automaticamente, para ver no MyAlbuns a versão atual sem reimportar o arquivo.
1. Como pessoa diagramadora, quero que o watcher espere uma gravação externa terminar, para não interpretar um arquivo temporariamente ilegível como falha definitiva.
1. Como pessoa diagramadora, quero que um original ausente seja restaurado automaticamente ao voltar ao mesmo caminho, para não religar um arquivo cuja localização não mudou.
1. Como pessoa diagramadora, quero atalhos e modificadores fixos e consistentes na primeira versão, para aprender comandos e gestos previsíveis.
1. Como pessoa diagramadora, quero manter fixos os gestos do mouse e o `Esc`, para sempre conseguir cancelar ou sair de um modo de maneira previsível.
1. Como pessoa diagramadora, quero que menus, dicas e gestos apresentem a mesma associação de cada comando, para não receber instruções divergentes.

### Janela do Projeto e navegação

1. Como pessoa diagramadora, quero menus desktop convencionais no topo, para encontrar Arquivo, edição, exibição, ferramentas e ajuda em locais previsíveis.
1. Como pessoa diagramadora, quero ver as Lâminas lado a lado em um Canvas contínuo, para percorrer o Álbum horizontalmente sem trocar de tela.
1. Como pessoa diagramadora, quero que todas as Lâminas do Canvas permaneçam interativas no modo normal, para trabalhar diretamente em qualquer uma sem ativá-la antes.
1. Como pessoa diagramadora, quero navegar em Álbuns longos sem um limite arbitrário de Lâminas, para que somente a região visível e sua margem de pré-carga consumam detalhe gráfico completo.
1. Como pessoa diagramadora, quero arrastar uma Foto para uma Lâmina específica e adicioná-la exatamente ali, para que o destino seja determinado pelo local da soltura.
1. Como pessoa diagramadora, quero arrastar um Decorativo diretamente para usá-lo como Background ou manter `Shift` pressionado para usá-lo como Overlay, para escolher a camada no próprio gesto.
1. Como pessoa diagramadora, quero soltar um Decorativo à esquerda, à direita ou no centro de uma Lâmina, para aplicá-lo respectivamente a um lado ou a Ambos os lados.
1. Como pessoa diagramadora, quero selecionar um Frame em qualquer Lâmina e receber imediatamente seu contexto, para alternar o trabalho com um único clique.
1. Como pessoa diagramadora, quero que comandos sem alvo explícito usem a Lâmina mais centralizada, para agir previsivelmente sem selecionar uma Lâmina antes.
1. Como pessoa diagramadora, quero pressionar `Enter` para editar a Lâmina centralizada, para entrar rapidamente no modo detalhado.
1. Como pessoa diagramadora, quero dar dois cliques em uma Foto do Painel para preencher o placeholder mais à esquerda ou, se não houver um, adicioná-la à Lâmina centralizada, para evitar arrastá-la por uma grande distância.
1. Como pessoa diagramadora, quero dar dois cliques em um Decorativo para aplicá-lo como Background a Ambos os lados da Lâmina centralizada ou usar `Shift` para Overlay, para aplicar rapidamente um design completo.
1. Como pessoa diagramadora, quero que o Modo de edição isole e amplie uma Lâmina, para concentrar a manipulação detalhada nela.
1. Como pessoa diagramadora, quero que o Painel de imagens fique temporariamente menor nesse modo, para aumentar o Canvas sem perder acesso às mídias.
1. Como pessoa diagramadora, quero retornar ao mesmo ponto e à mesma altura anterior do painel ao sair, para continuar a navegação sem reorganizar a interface.
1. Como pessoa diagramadora, quero uma Grade de Lâminas no Painel contextual, para obter uma visão geral, saltar rapidamente e reordenar a sequência por miniaturas.
1. Como pessoa diagramadora, quero o Painel de imagens abaixo do Canvas e ocupando toda a largura restante até o Painel contextual, para manter mídias próximas da composição.
1. Como pessoa diagramadora, quero redimensionar ou ocultar o Painel de imagens, para priorizar mídias ou ampliar o Canvas conforme a tarefa.
1. Como pessoa diagramadora, quero redimensionar o Painel contextual por um divisor vertical, para equilibrar ferramentas e área de composição.
1. Como pessoa diagramadora, quero ocultar o Painel contextual, para dedicar toda a largura à composição quando não precisar das ferramentas.
1. Como pessoa diagramadora, quero um único Painel contextual com seções recolhíveis, para reutilizar o espaço sem abrir janelas auxiliares.
1. Como pessoa diagramadora, quero consultar um resumo em Informações do Álbum e alterar todas as configurações globais do Projeto em Design do Álbum, para encontrar cada responsabilidade em um local previsível.
1. Como pessoa diagramadora, quero que selecionar um Frame ou Foto troque o Painel para preview, informações, Design e Ajustes e Efeitos, para editar o elemento no mesmo local.
1. Como pessoa diagramadora, quero restaurar um slider com dois cliques, para voltar rapidamente ao valor padrão sem ocupar espaço com outro botão.

### Criação e configuração

1. Como pessoa diagramadora, quero criar um Projeto que represente exatamente um Álbum, para manter cada trabalho como uma unidade independente.
1. Como pessoa diagramadora, quero definir largura e altura físicas da Lâmina, para trabalhar no formato contratado para o álbum.
1. Como pessoa diagramadora, quero ver a Página calculada como metade da largura da Lâmina e com a mesma altura, para manter as superfícies coerentes.
1. Como pessoa diagramadora, quero escolher `mm`, `cm` ou `in` como Unidade do Projeto, para informar todas as medidas no padrão que utilizo.
1. Como pessoa diagramadora, quero definir a Resolução do Projeto em DPI, para controlar a resolução das saídas.
1. Como pessoa diagramadora, quero iniciar com 300 DPI quando não escolher outro valor, para ter um padrão adequado à produção.
1. Como pessoa diagramadora, quero escolher a quantidade inicial de Lâminas, para começar com a estrutura desejada.
1. Como pessoa diagramadora, quero escolher independentemente se as extremidades serão duplas ou de página única, para representar diferentes modelos de álbum.
1. Como pessoa diagramadora, quero configurar os padrões iniciais de Background, Overlay e Frame, para que novos elementos adotem a identidade visual do Projeto.
1. Como pessoa diagramadora, quero começar sem personalizações com duas Lâminas duplas, Background branco, sem Overlay, borda, Frame ou Foto, para receber um Projeto válido e neutro.
1. Como pessoa diagramadora, quero informar Sangria e segurança na Unidade do Projeto, para manter todas as medidas físicas consistentes.
1. Como pessoa diagramadora, quero que Sangria e segurança comecem no equivalente físico a `3 mm`, para ter valores iniciais consistentes entre Unidades.
1. Como pessoa diagramadora, quero definir Sangria ou segurança como zero separadamente, para desativar a guia que não utilizarei.
1. Como pessoa diagramadora, quero escolher Nome e Localização em um diálogo nativo depois de configurar o Projeto, para decidir onde o arquivo será criado.
1. Como pessoa diagramadora, quero voltar às configurações preenchidas ao cancelar o diálogo nativo, para não refazer minhas escolhas.
1. Como pessoa diagramadora, quero que cancelar a criação não deixe arquivo incompleto, para manter o destino limpo.
1. Como pessoa diagramadora, quero trocar a Unidade depois com conversão de todos os valores, para mudar sua representação sem alterar tamanhos reais.
1. Como pessoa diagramadora, quero alterar o DPI posteriormente, para adaptar a renderização sem mudar as dimensões físicas.
1. Como pessoa diagramadora, quero alterar Sangria e segurança posteriormente sem mover conteúdo, para ajustar o acabamento sem desorganizar o Álbum.
1. Como pessoa diagramadora, quero mudar a Dimensão da Lâmina dentro de uma faixa segura, para adaptar a composição a um formato proporcionalmente compatível.
1. Como pessoa diagramadora, quero que Frames, Pan, Zoom e ponto focal sejam transformados proporcionalmente após essa mudança, para preservar a composição.
1. Como pessoa diagramadora, quero que uma mudança dimensional excessivamente diferente seja bloqueada, para não receber um resultado cuja qualidade não possa ser garantida.

### Lâminas, Páginas e acabamento

1. Como pessoa diagramadora, quero que todo Álbum tenha ao menos duas Lâminas, para conservar uma extremidade inicial e uma final.
1. Como pessoa diagramadora, quero que uma Lâmina dupla tenha Páginas ativas à esquerda e à direita, para diagramar a abertura completa.
1. Como pessoa diagramadora, quero que a Lâmina inicial de página única tenha somente o lado direito ativo, para representar corretamente sua posição.
1. Como pessoa diagramadora, quero que a Lâmina final de página única tenha somente o lado esquerdo ativo, para representar corretamente sua posição.
1. Como pessoa diagramadora, quero que Lâminas internas sejam sempre duplas, para manter ambos os lados ativos no interior do Álbum.
1. Como pessoa diagramadora, quero que o lado inativo não contenha Página, conteúdo ou interação, para não confundi-lo com uma página vazia.
1. Como pessoa diagramadora, quero reordenar Lâminas e atualizar seus papéis pela posição, para reorganizar a sequência do Álbum.
1. Como pessoa diagramadora, quero arrastar uma Lâmina por sua Barra usando um espaço reservado móvel e um fantasma visual, para compreender a posição resultante antes de confirmar.
1. Como pessoa diagramadora, quero adicionar antes/depois, duplicar, excluir ou converter uma extremidade pelo menu `Lâmina`, para editar a estrutura usando a Lâmina centralizada.
1. Como pessoa diagramadora, quero os mesmos comandos no menu de contexto, para atuar explicitamente sobre a Lâmina clicada.
1. Como pessoa diagramadora, quero que uma Lâmina adicionada nasça dupla, vazia e herdando o design atual do Álbum, para receber uma base previsível sem composição artificial.
1. Como pessoa diagramadora, quero duplicar uma Lâmina imediatamente depois da original com toda a composição, para reutilizar rapidamente uma base já pronta.
1. Como pessoa diagramadora, quero que a Lâmina duplicada seja independente e apenas reutilize os mesmos vínculos externos, para editá-la sem alterar a origem ou copiar arquivos.
1. Como pessoa diagramadora, quero impedir a duplicação de uma extremidade de Página única, para não criar uma Página única inválida no interior do Álbum.
1. Como pessoa diagramadora, quero impedir inserções que desloquem uma extremidade de Página única para o interior, para preservar uma sequência válida.
1. Como pessoa diagramadora, quero excluir uma Lâmina sem confirmação e restaurá-la integralmente por Undo, para editar a sequência com agilidade e segurança.
1. Como pessoa diagramadora, quero manter Fotos e Decorativos importados após excluir seus usos em uma Lâmina, para não apagar recursos do Painel involuntariamente.
1. Como pessoa diagramadora, quero que uma Página única permaneça presa à extremidade correspondente, para impedir uma sequência inválida.
1. Como pessoa diagramadora, quero impedir reordenações indiretas que empurrem uma página única para o interior, para conservar a estrutura válida.
1. Como pessoa diagramadora, quero excluir uma Lâmina enquanto restarem ao menos duas, para reduzir o Álbum sem eliminar suas extremidades.
1. Como pessoa diagramadora, quero que a vizinha assuma o papel da extremidade excluída, para preservar automaticamente uma sequência válida.
1. Como pessoa diagramadora, quero converter uma extremidade entre dupla e página única, para adaptar o formato do início ou do fim.
1. Como pessoa diagramadora, quero preservar Fotos, placeholders e estilos durante essa conversão, para não perder o conteúdo escolhido.
1. Como pessoa diagramadora, quero que Frames existentes recebam o primeiro Layout compatível e destravado após a conversão, para se reorganizarem na nova superfície.
1. Como pessoa diagramadora, quero que uma Lâmina sem Frames continue sem Layout após a conversão, para não criar estrutura desnecessária.
1. Como pessoa diagramadora, quero que uma redução preserve apenas o Background e Overlay do lado que continua ativo quando o escopo for por lado, para eliminar conteúdo sem superfície.
1. Como pessoa diagramadora, quero que aplicações de Ambos os lados sejam reajustadas à superfície ativa após uma redução ou expansão, para continuar preenchendo sua área.
1. Como pessoa diagramadora, quero que um lado recém-ativado em aplicação por lado comece no padrão atual, para receber um estado previsível.
1. Como pessoa diagramadora, quero que Sangria e segurança consumam espaço dentro da Dimensão informada, para não alterar o tamanho do Projeto ou da saída.
1. Como pessoa diagramadora, quero usar um valor uniforme de Sangria e outro de segurança, para configurar o acabamento de forma simples.
1. Como pessoa diagramadora, quero que a segurança seja medida adicionalmente a partir da linha de corte, para acumular corretamente os recuos.
1. Como pessoa diagramadora, quero que a divisão central de uma Lâmina dupla não receba margens, para tratá-la apenas como separação entre Páginas.
1. Como pessoa diagramadora, quero que a borda voltada ao lado inativo de uma página única não receba margens, para não criar acabamento onde estaria a outra Página.
1. Como pessoa diagramadora, quero ver normalmente apenas a Área de corte, para avaliar a aparência provável após o acabamento.
1. Como pessoa diagramadora, quero ver toda a superfície ativa e as guias no Modo de edição, para ajustar também conteúdo situado na Sangria sem reativar o lado inativo.
1. Como pessoa diagramadora, quero atravessar livremente as guias com conteúdo, para tratá-las como orientação e não como bloqueio.
1. Como pessoa diagramadora, quero que alterações de Sangria e segurança entrem no Undo/Redo e deixem mudanças pendentes, para controlar sua persistência.
1. Como pessoa diagramadora, quero bloquear margens que eliminem a Área de corte ou segurança de alguma Página ativa, para evitar uma configuração impossível.

### Background, Overlay e herança

1. Como pessoa diagramadora, quero que toda Lâmina tenha Background obrigatório e branco por padrão, para nunca existir base indefinida.
1. Como pessoa diagramadora, quero usar uma cor ou imagem como Background, para construir a base visual desejada.
1. Como pessoa diagramadora, quero aplicar ou omitir um Overlay, para acrescentar uma máscara frontal somente quando necessário.
1. Como pessoa diagramadora, quero preservar a transparência do Overlay, para revelar as camadas inferiores nas regiões transparentes.
1. Como pessoa diagramadora, quero escolher independentemente o escopo do Background e do Overlay, para combinar aplicações de Ambos os lados e por lado.
1. Como pessoa diagramadora, quero aplicar uma única imagem aos lados ativos de uma Lâmina, para criar uma composição contínua.
1. Como pessoa diagramadora, quero configurar cada lado separadamente, para personalizar as duas Páginas.
1. Como pessoa diagramadora, quero que imagens de Background e Overlay preencham exatamente sua área, para não deixar regiões descobertas.
1. Como pessoa diagramadora, quero um comportamento inicial sem Pan ou Zoom nessas camadas, para que seu ajuste seja simples e previsível.
1. Como pessoa diagramadora, quero alterar o Padrão visual e atualizar somente aplicações herdadas, para mudar o visual global sem destruir exceções.
1. Como pessoa diagramadora, quero que uma edição manual transforme o alvo em personalizado, para protegê-lo de mudanças globais posteriores.
1. Como pessoa diagramadora, quero restaurar uma personalização ao padrão atual, para fazê-la voltar a herdar mudanças.
1. Como pessoa diagramadora, quero remover localmente um Overlay herdado como ausência personalizada, para que ele permaneça ausente diante de novos padrões.
1. Como pessoa diagramadora, quero remover localmente um Background herdado como branco personalizado, para manter uma base válida e independente.
1. Como pessoa diagramadora, quero personalizar somente um lado de uma aplicação contínua, para conservar herança no outro lado.
1. Como pessoa diagramadora, quero que Background, Frames/Fotos e Overlay sejam renderizados nessa ordem fixa, para obter uma composição previsível.
1. Como pessoa diagramadora, quero reordenar Frames somente entre si, para controlar sobreposições sem quebrar a Pilha visual.

### Frames, Fotos e edição

1. Como pessoa diagramadora, quero que toda Foto colocada esteja dentro de um Frame, para limitar sempre sua área visível.
1. Como pessoa diagramadora, quero usar o mesmo arquivo em vários Frames com ajustes independentes, para criar ocorrências diferentes sem duplicar a mídia.
1. Como pessoa diagramadora, quero que cada Frame contenha no máximo uma Foto, para manter clara a relação entre máscara e imagem.
1. Como pessoa diagramadora, quero preencher um placeholder sem mudar sua geometria, para completar uma posição já reservada.
1. Como pessoa diagramadora, quero que adicionar uma Foto em área livre crie um Frame, para colocá-la na composição.
1. Como pessoa diagramadora, quero criar manualmente um placeholder somente no Modo de edição, para reservar espaço sem inserir uma Foto.
1. Como pessoa diagramadora, quero manter uma Lâmina sem Frames como válida, para exportar composições formadas somente pelas camadas fixas.
1. Como pessoa diagramadora, quero que um placeholder bloqueie qualquer Exportação que inclua sua Lâmina, para evitar espaços involuntariamente vazios.
1. Como pessoa diagramadora, quero que todo Frame seja retangular, alinhado aos eixos e contido na área ativa, para trabalhar apenas com a geometria suportada.
1. Como pessoa diagramadora, quero atravessar o centro com um Frame somente em Lâmina dupla, para criar composições contínuas sem ocupar lado inativo.
1. Como pessoa diagramadora, quero mover e redimensionar Frames destravados, para ajustar livremente a composição.
1. Como pessoa diagramadora, quero excluir conjuntamente um Frame destravado e sua Foto, para remover o elemento completo.
1. Como pessoa diagramadora, quero que a exclusão fora do Modo de edição aplique o primeiro Layout da quantidade restante, para reorganizar automaticamente a Lâmina.
1. Como pessoa diagramadora, quero que a exclusão dentro do Modo de edição preserve os demais Frames, para continuar meu ajuste manual.
1. Como pessoa diagramadora, quero remover a Foto de um Frame travado e manter o placeholder, para conservar a estrutura fixada.
1. Como pessoa diagramadora, quero destravar antes de excluir uma estrutura de Frame travada, para tornar explícita a mudança geométrica.
1. Como pessoa diagramadora, quero substituir a Foto preservando Frame e estilo, para trocar a imagem sem reconstruir a composição.
1. Como pessoa diagramadora, quero que a substituta comece centralizada, colorida, sem espelho, em `0°` e com Zoom do usuário em `1×`, para receber um estado previsível.
1. Como pessoa diagramadora, quero ajustar Pan e Zoom de uma Foto, para escolher seu enquadramento dentro do Frame.
1. Como pessoa diagramadora, quero usar `Alt` + arrastar e `Alt` + roda no modo normal para ajustar diretamente o Pan e o Zoom da Foto sob o ponteiro.
1. Como pessoa diagramadora, quero que os gestos diretos no Modo de edição manipulem Frames, para separar claramente geometria de enquadramento.
1. Como pessoa diagramadora, quero selecionar vários Frames no Modo de edição, para executar operações coletivas sem repetir cada ação individualmente.
1. Como pessoa diagramadora, quero copiar e colar uma Seleção de Frames com Fotos, estilos e ajustes, para reutilizar uma composição sem reconstruí-la.
1. Como pessoa diagramadora, quero que a colagem reutilize os vínculos com os Arquivos originais e forme uma única ação de Undo/Redo, para duplicar a composição sem duplicar mídia.
1. Como pessoa diagramadora, quero trocar as Fotos de dois Frames selecionados sem mover suas geometrias ou estilos, para reorganizar rapidamente a composição.
1. Como pessoa diagramadora, quero mover uma Foto para um placeholder pela mesma troca, para preservar ambos os Frames.
1. Como pessoa diagramadora, quero colar Frames em outra Lâmina do mesmo Projeto, para reaproveitar uma composição mantendo suas posições.
1. Como pessoa diagramadora, quero que a colagem na mesma Lâmina use o maior deslocamento seguro até o limite desejado e, sem espaço para deslocar, crie os Frames na mesma posição, selecionados e acima dos originais em ordem determinística.
1. Como pessoa diagramadora, quero que Frames copiados sejam adaptados proporcionalmente ao colar em uma Página única, para aproveitar a composição sem ultrapassar sua área ativa.
1. Como pessoa diagramadora, quero preservar o lado lógico ao colar de uma Página única em uma Lâmina dupla, para que uma composição direita continue à direita e uma esquerda continue à esquerda.
1. Como pessoa diagramadora, quero que a cópia de Frames não atravesse Projetos, para preservar o isolamento entre suas sessões.
1. Como pessoa diagramadora, quero que o Zoom nunca revele áreas vazias, para preservar o preenchimento integral.
1. Como pessoa diagramadora, quero espelhar horizontalmente uma Foto, para inverter sua composição sem alterar o original.
1. Como pessoa diagramadora, quero girar a Foto em passos anti-horários de `90°`, para corrigir sua orientação de forma previsível.
1. Como pessoa diagramadora, quero inclinar a Foto entre `-45°` e `+45°` independentemente do Giro de 90°, para realizar um ajuste fino de composição.
1. Como pessoa diagramadora, quero que o Giro e o Ângulo recalculem o preenchimento, para que sua combinação nunca revele áreas vazias no Frame.
1. Como pessoa diagramadora, quero alternar preto e branco, para aplicar um tratamento reversível.
1. Como pessoa diagramadora, quero preservar Pan, Zoom adicional e ponto focal quando o Frame mudar, para manter o enquadramento sempre que possível.
1. Como pessoa diagramadora, quero limitar ajustes apenas quando necessário para impedir vazamentos, para não perder todo o enquadramento.
1. Como pessoa diagramadora, quero que todos os ajustes sejam não destrutivos, para nunca modificar o arquivo original.
1. Como pessoa diagramadora, quero configurar uma Borda interna com cor e espessura física, para personalizar a apresentação do Frame.
1. Como pessoa diagramadora, quero que a Borda preserve a geometria externa, para não alterar a posição usada pelos Layouts.
1. Como pessoa diagramadora, quero definir Opacidade conjunta para Foto e Borda, para revelar as camadas inferiores.
1. Como pessoa diagramadora, quero que novos Frames herdem o Padrão de Frame, para manter consistência visual.
1. Como pessoa diagramadora, quero que editar Borda ou Opacidade torne o estilo inteiro personalizado, para congelar minha exceção local.
1. Como pessoa diagramadora, quero restaurar o Frame ao padrão atual e Opacidade `100%`, para fazê-lo retomar a herança.

### Layouts

1. Como pessoa diagramadora, quero navegar normalmente por Layouts com a mesma quantidade de Frames, para comparar organizações diretamente aplicáveis.
1. Como pessoa diagramadora, quero que Barra e Painel de Layouts existam somente no modo normal, para manter o Modo de edição focado na manipulação manual dos Frames.
1. Como pessoa diagramadora, quero que um Layout armazene somente geometrias ordenadas, para não substituir Fotos, estilos ou camadas fixas.
1. Como pessoa diagramadora, quero clicar na prévia para aplicar a geometria uma única vez, para reorganizar e continuar editando livremente.
1. Como pessoa diagramadora, quero visualizar no hover meus próprios Frames, Fotos, estilos e enquadramentos na geometria candidata, para avaliar o resultado antes de aplicá-lo.
1. Como pessoa diagramadora, quero usar o cadeado da prévia para aplicar e travar na mesma ação, para fixar a estrutura sem abrir outra tela.
1. Como pessoa diagramadora, quero que o travamento congele quantidade, posição e dimensões dos Frames, para preservar a estrutura enquanto continuo ajustando Fotos e estilos.
1. Como pessoa diagramadora, quero reconhecer a preview travada pelo destaque e pelo cadeado fechado, para identificar imediatamente a organização protegida.
1. Como pessoa diagramadora, quero que outras previews fiquem desabilitadas enquanto a Lâmina estiver travada, para não substituir a estrutura acidentalmente.
1. Como pessoa diagramadora, quero destravar sem alterar a composição, para retomar a edição geométrica sem perder conteúdo.
1. Como pessoa diagramadora, quero que o Mapeamento preserve Fotos, ausências e estilos pela ordem visual, para mudar somente a geometria.
1. Como pessoa diagramadora, quero que placeholders novos herdem o Padrão de Frame, para manter consistência com o Projeto.
1. Como pessoa diagramadora, quero travar um Layout com a mesma quantidade de posições, para reorganizar e fixar a estrutura existente.
1. Como pessoa diagramadora, quero que posições excedentes de um Layout travado virem placeholders, para reservar todos os espaços.
1. Como pessoa diagramadora, quero rejeitar Layout travado com menos posições que Frames, para não perder elementos existentes.
1. Como pessoa diagramadora, quero adicionar ou excluir Frames fora do Modo de edição e receber o primeiro Layout compatível, para automatizar a reorganização.
1. Como pessoa diagramadora, quero inserir e excluir no Modo de edição sem aplicação automática de Layout, para preservar minha organização manual.
1. Como pessoa diagramadora, quero que uma nova ocorrência preencha apenas placeholders quando a Lâmina estiver travada, mas permitir substituição explícita em Frame preenchido, para preservar sua estrutura sem impedir a troca de Foto.
1. Como pessoa diagramadora, quero que o último Layout aplicado apareça primeiro enquanto compatível, para reencontrar rapidamente a organização usada.
1. Como pessoa diagramadora, quero reaplicar a geometria original do último Layout depois de ajustes manuais, para voltar à organização selecionada.
1. Como pessoa diagramadora, quero que a Organização aplicada seja uma cópia local, para que mudanças no catálogo não alterem minha Lâmina.
1. Como pessoa diagramadora, quero encontrar Layouts gerados em `Automáticos` e Layouts criados por usuários em `Personalizados`, para reconhecer a origem de cada organização.
1. Como pessoa diagramadora, quero que favoritar não mude a categoria do Layout, para continuar reconhecendo sua origem.
1. Como pessoa diagramadora, quero ver último aplicado, favoritos e demais candidatos nessa ordem dentro de cada seção, para localizar rapidamente as opções mais relevantes sem previews duplicadas.
1. Como pessoa diagramadora, quero priorizar último aplicado, favoritos, personalizados globais e sistema, para receber primeiro as opções mais relevantes.
1. Como pessoa diagramadora, quero reutilizar Layouts entre medidas, Unidades ou DPIs compatíveis, para aproveitar a mesma proporção por escala.
1. Como pessoa diagramadora, quero separar Layouts de página única e Lâmina dupla, para impedir geometrias incompatíveis.
1. Como pessoa diagramadora, quero usar Layouts por Lâmina que atravessem o centro, para criar organizações contínuas.
1. Como pessoa diagramadora, quero usar Layouts por Página que nunca atravessem o centro, para manter cada Frame integralmente em um lado.
1. Como pessoa diagramadora, quero que Layouts por Página centralizem globalmente seus Blocos de Frames, para equilibrar a composição.
1. Como pessoa diagramadora, quero salvar imediatamente a geometria atual como Layout personalizado global no Modo de edição, sem informar um nome, para reutilizá-la em outros Projetos pela própria preview.
1. Como pessoa diagramadora, quero que cada Janela consulte o catálogo global ao abrir o Painel de Layouts, receber foco ou solicitar atualização, para ver mudanças persistidas sem exigir sincronização instantânea entre processos.
1. Como pessoa diagramadora, quero impedir Layouts sem Frames e organizações geométricas ordenadas duplicadas, para manter `Personalizados` útil sem confundir sequências visuais diferentes.
1. Como pessoa diagramadora, quero inferir automaticamente o escopo do Layout pela Travessia central, para não classificá-lo manualmente.
1. Como pessoa diagramadora, quero excluir um Layout personalizado pela lixeira mediante confirmação, para removê-lo do catálogo global sem apagar composições ou favoritos existentes.
1. Como pessoa diagramadora, quero favoritar um Layout como cópia estável no Projeto, para priorizá-lo somente naquele Álbum.
1. Como pessoa diagramadora, quero favoritar ou desfavoritar sem confirmação e com Undo/Redo, para controlar rapidamente a prioridade sem perder a ação anterior.
1. Como pessoa diagramadora, quero conservar um favorito mesmo se sua origem global mudar ou desaparecer, para não perder uma organização local.
1. Como pessoa diagramadora, quero que o Gerador de Layouts sempre forneça ao menos uma opção compatível, para que operações automáticas nunca fiquem sem solução.

### Painel de imagens e vínculos

1. Como pessoa diagramadora, quero alternar entre `Fotos` e `Decorativos`, para separar imagens fotográficas dos recursos de Background e Overlay.
1. Como pessoa diagramadora, quero que a aba ativa determine a categoria da importação, para classificar o arquivo pela minha intenção.
1. Como pessoa diagramadora, quero importar um ou vários arquivos em uma única seleção, para alimentar o Painel sem repetir o diálogo.
1. Como pessoa diagramadora, quero importar as imagens diretamente contidas em uma pasta, para adicionar um conjunto conhecido sem misturar suas subpastas.
1. Como pessoa diagramadora, quero arrastar arquivos ou pastas do sistema operacional para o Painel, para importar pelo gesto mais conveniente.
1. Como pessoa diagramadora, quero importar JPG/JPEG, PNG e TIFF/TIF, para trabalhar com os formatos suportados inicialmente.
1. Como pessoa diagramadora, quero rejeitar claramente formatos não suportados, para não acreditar que foram importados.
1. Como pessoa diagramadora, quero preservar os arquivos válidos quando outros falharem na mesma importação, para não repetir uma operação grande por causa de um item.
1. Como pessoa diagramadora, quero ver cada arquivo rejeitado e seu motivo, para entender o resultado parcial.
1. Como pessoa diagramadora, quero desfazer uma importação inteira como uma única ação, para retirar rapidamente o conjunto adicionado por engano.
1. Como pessoa diagramadora, quero que Undo da importação remova somente os vínculos criados, para nunca alterar os arquivos originais ou itens que já existiam.
1. Como pessoa diagramadora, quero manter o mesmo caminho uma vez em cada aba, para usar o arquivo em papéis diferentes.
1. Como pessoa diagramadora, quero que reimportar na mesma aba apenas selecione o item existente, para evitar duplicatas.
1. Como pessoa diagramadora, quero buscar em tempo real pelo Nome do arquivo na aba atual, para localizar rapidamente uma mídia sem alterar sua organização.
1. Como pessoa diagramadora, quero combinar a busca com os filtros de uso, para restringir os resultados por texto e utilização ao mesmo tempo.
1. Como pessoa diagramadora, quero ajustar o tamanho das miniaturas e ver a grade se reorganizar imediatamente, para equilibrar identificação visual e quantidade de itens.
1. Como pessoa diagramadora, quero preservar a proporção inteira das imagens nas miniaturas, para reconhecê-las sem cortes.
1. Como pessoa diagramadora, quero selecionar mídias individualmente ou por intervalo no Painel, para preparar ações em lote sem marcar uma a uma.
1. Como pessoa diagramadora, quero usar `Ctrl + A` para selecionar somente os resultados visíveis, para executar ações em lote respeitando minha busca e meus filtros.
1. Como pessoa diagramadora, quero que mídias ocultadas por uma busca ou filtro saiam da seleção, para nunca executar uma ação em lote sobre itens invisíveis.
1. Como pessoa diagramadora, quero remover a seleção do Painel por `Delete` ou pelo menu de contexto, para usar o comando mais conveniente.
1. Como pessoa diagramadora, quero evitar uma Caixa de seleção no Painel, para manter a interação simples na grade de imagens.
1. Como pessoa diagramadora, quero ordenar por Nome, Data de criação ou Data de alteração, para encontrar imagens por critérios diferentes.
1. Como pessoa diagramadora, quero escolher direção crescente ou decrescente, para adaptar a visualização ao meu fluxo.
1. Como pessoa diagramadora, quero que Nome use ordenação natural como `1`, `2`, `10`, para acompanhar sequências intuitivamente.
1. Como pessoa diagramadora, quero que datas venham dos arquivos originais, ausentes fiquem no fim e empates usem Nome, para obter ordem determinística.
1. Como pessoa diagramadora, quero filtrar por `Todas`, `Usadas` ou `Não usadas`, para localizar mídias posicionadas ou disponíveis.
1. Como pessoa diagramadora, quero considerar uma Foto usada quando aparecer em algum Frame, para que o filtro reflita a composição.
1. Como pessoa diagramadora, quero considerar um Decorativo usado quando aplicado ou definido como padrão, para reconhecer todas as referências relevantes.
1. Como pessoa diagramadora, quero calcular o uso separadamente nas duas abas, para não misturar papéis do mesmo arquivo.
1. Como pessoa diagramadora, quero conservar ordenação e filtro por aba entre Projetos e sessões, para manter minhas preferências.
1. Como pessoa diagramadora, quero que essas preferências não alterem o Projeto ou Undo/Redo, para tratá-las apenas como estado da interface.
1. Como pessoa diagramadora, quero manter imagens vinculadas ao caminho original, para evitar duplicar arquivos grandes.
1. Como pessoa diagramadora, quero usar Projetos, mídias e destinos em discos locais, compartilhamentos UNC, unidades mapeadas e caminhos longos, para trabalhar diretamente com a organização real dos arquivos.
1. Como pessoa diagramadora, quero que uma origem de rede indisponível não seja confundida com arquivo removido, para não religar ou alterar referências por uma falha temporária.
1. Como pessoa diagramadora, quero que operações com muitos arquivos na mesma origem de rede reutilizem a resolução dessa raiz durante a execução, para evitar trabalho repetitivo sem criar estado permanente.
1. Como pessoa diagramadora, quero observar a nova versão quando o conteúdo do mesmo caminho for substituído, para trabalhar com o original atual.
1. Como pessoa diagramadora, quero relocalizar uma referência ausente somente no Projeto atual, para preservar o isolamento entre trabalhos.
1. Como pessoa diagramadora, quero receber aviso sobre arquivo ausente que esteja apenas no Painel, para corrigi-lo sem bloquear saída que não o utiliza.
1. Como pessoa diagramadora, quero bloquear a Exportação se um original necessário à seleção estiver ausente, para não gerar resultado incompleto pelo Cache.
1. Como pessoa diagramadora, quero remover Foto em uso escolhendo remover tudo, manter Frames como placeholders ou cancelar, para controlar o impacto.
1. Como pessoa diagramadora, quero aplicar uma única decisão ao remover várias Fotos, para não responder ao mesmo diálogo repetidamente.
1. Como pessoa diagramadora, quero que `Remover tudo` exclua Frames destravados e preserve Frames travados como placeholders, para respeitar cada estrutura.
1. Como pessoa diagramadora, quero confirmar a remoção de um Decorativo, para evitar retirar acidentalmente um recurso aplicado.
1. Como pessoa diagramadora, quero confirmar uma seleção de Decorativos em conjunto, para remover o lote sem diálogos individuais.
1. Como pessoa diagramadora, quero que usos do Decorativo removido voltem ao padrão, para manter aplicações válidas.
1. Como pessoa diagramadora, quero que remover o Decorativo padrão torne o Background branco ou o Overlay ausente somente nos herdados, para preservar personalizações.
1. Como pessoa diagramadora, quero que o Cache acelere a interação sem participar da saída final, para combinar fluidez e qualidade.
1. Como pessoa diagramadora, quero reutilizar metadados e uma representação reduzida de cada mídia, para abrir Projetos e navegar pelo Painel sem reler os originais desnecessariamente.
1. Como pessoa diagramadora, quero invalidar o Cache quando o original mudar, para não continuar vendo uma versão desatualizada.
1. Como pessoa diagramadora, quero que o Cache nunca torne válido um original ausente, para não confundir prévia temporária com mídia final.

### Identidade, salvamento e recuperação

1. Como pessoa diagramadora, quero que o Nome exibido e usado na Exportação acompanhe o arquivo do Projeto, para reconhecer suas saídas.
1. Como pessoa diagramadora, quero mover ou renomear o arquivo preservando a Identidade, para reorganizar meus arquivos sem criar outro Projeto.
1. Como pessoa diagramadora, quero que um Projeto movido reutilize seu Cache, para não reconstruir dados apenas porque o caminho mudou.
1. Como pessoa diagramadora, quero copiar um Projeto com toda sua estrutura e abrir original e cópia simultaneamente, para trabalhar em versões independentes.
1. Como pessoa diagramadora, quero que uma cópia não mantenha herança ou sincronização, para que editar uma versão nunca altere a outra.
1. Como pessoa diagramadora, quero que `Salvar como` e Cópias externas usem Caches independentes, para que uma versão nunca dependa da outra.
1. Como pessoa diagramadora, quero que uma cópia feita pelo sistema operacional receba nova Identidade quando o original existir, para funcionar como outro Projeto.
1. Como pessoa diagramadora, quero que a correção automática de Identidade não salve alterações pendentes, para manter controle sobre meu conteúdo.
1. Como pessoa diagramadora, quero que uma Cópia externa sem permissão de escrita não compartilhe Cache ou Recuperação com o original e ofereça `Salvar cópia como...`, para falhar com segurança.
1. Como pessoa diagramadora, quero que um Projeto apenas movido preserve sua Identidade, para não ser confundido com uma cópia.
1. Como pessoa diagramadora, quero que abrir novamente o mesmo Projeto focalize sua sessão existente, para evitar duas edições concorrentes.
1. Como pessoa diagramadora, quero que abrir o mesmo Projeto por unidade mapeada ou UNC focalize a sessão existente, para não tratar dois nomes do mesmo arquivo como cópias.
1. Como pessoa diagramadora, quero que uma identidade física inconclusiva bloqueie uma segunda edição em vez de presumir uma cópia, para proteger o Projeto.
1. Como pessoa diagramadora, quero abrir Projetos independentes que compartilhem arquivos externos, para trabalhar em paralelo sem compartilhar estado.
1. Como pessoa diagramadora, quero recuperar automaticamente um Bloqueio órfão, para não perder acesso após uma falha.
1. Como pessoa diagramadora, quero salvar alterações criativas somente por ação explícita, para controlar a versão persistida.
1. Como pessoa diagramadora, quero escolher salvar, descartar ou cancelar ao fechar com mudanças, para controlar o encerramento.
1. Como pessoa diagramadora, quero usar `Salvar como` para criar novo Nome, Localização e Identidade a partir do estado visível, para iniciar uma versão independente.
1. Como pessoa diagramadora, quero que a sessão passe ao novo Projeto e o original fique na última versão salva, para preservar a versão anterior.
1. Como pessoa diagramadora, quero recuperar estado temporário após falha, para reduzir perda de trabalho.
1. Como pessoa diagramadora, quero que uma sessão recuperada continue não salva, para decidir se deve substituir o arquivo.
1. Como pessoa diagramadora, quero que cada ação concluída atualize a recuperação temporária sem alterar meu arquivo, para limitar a perda de trabalho após uma falha.
1. Como pessoa diagramadora, quero que uma queda durante um gesto restaure o estado anterior ao gesto, para nunca recuperar uma transformação parcial.
1. Como pessoa diagramadora, quero recuperar o estado criativo consolidado e o marco da última versão salva, para retomar o conteúdo sem depender da integridade do Histórico.
1. Como pessoa diagramadora, quero que Undo e Redo comecem vazios depois de uma recuperação, para não persistir uma pilha complexa e frágil entre processos.
1. Como pessoa diagramadora, quero que sessões longas mantenham um Histórico dentro de limites seguros de memória, para não degradar indefinidamente o aplicativo.
1. Como pessoa diagramadora, quero que a remoção de ações antigas nunca altere meu Projeto nem seu estado de Salvamento, para que limitar memória não mude o trabalho.
1. Como pessoa diagramadora, quero que a falha de uma Sessão do Projeto nunca corrompa o estado nem a Recuperação de outros Projetos, para manter trabalhos independentes isolados.
1. Como pessoa diagramadora, quero escolher entre recuperar, abrir a última versão salva ou decidir depois, para controlar o que acontece após uma falha.
1. Como pessoa diagramadora, quero que decidir depois preserve a recuperação temporária, para não perder a oportunidade de restaurá-la.
1. Como pessoa diagramadora, quero desfazer e refazer todas as alterações editáveis, para explorar e corrigir decisões.
1. Como pessoa diagramadora, quero conservar Undo e Redo depois de Salvar, para que o Salvamento não interrompa meu fluxo.
1. Como pessoa diagramadora, quero encerrar o histórico ao fechar a sessão, para não misturar sessões diferentes.
1. Como pessoa diagramadora, quero que qualquer edição permaneça restrita ao Projeto atual, para nunca alterar outro trabalho.

### Exportação normal

1. Como pessoa diagramadora, quero exportar o estado atual visível, inclusive mudanças não salvas, para que a saída corresponda ao que estou vendo.
1. Como pessoa diagramadora, quero que exportar não salve o Projeto, para manter separadas as duas decisões.
1. Como pessoa diagramadora, quero somente uma Exportação normal ativa entre todos os Projetos, para manter o uso de recursos e o fluxo previsíveis.
1. Como pessoa diagramadora, quero que outro Projeto não enfileire uma Exportação normal enquanto uma estiver ativa, para evitar uma fila global que eu precise administrar.
1. Como pessoa diagramadora, quero continuar editando e salvando outros Projetos durante uma Exportação, para que o bloqueio não interrompa trabalhos independentes.
1. Como pessoa diagramadora, quero que o progresso apareça em um modal pertencente somente ao Projeto exportado, para não bloquear nem cobrir as outras janelas.
1. Como pessoa diagramadora, quero renderizar sempre os originais, para obter a qualidade final e não uma representação do Cache.
1. Como pessoa diagramadora, quero exportar o Álbum inteiro ou um intervalo contínuo de Lâminas, para gerar todo o trabalho ou uma seleção.
1. Como pessoa diagramadora, quero acionar `Exportar Lâmina` e abrir o intervalo preenchido com ela, para exportá-la rapidamente.
1. Como pessoa diagramadora, quero validar dependências efetivamente renderizadas somente na seleção, para que problemas externos não bloqueiem a saída.
1. Como pessoa diagramadora, quero exportar em JPEG, PNG ou PDF, para escolher o formato necessário.
1. Como pessoa diagramadora, quero ajustar a qualidade ao exportar um único Projeto em JPEG, para controlar a compactação sem adicionar opções irrelevantes aos outros formatos.
1. Como pessoa diagramadora, quero exportar `Por lâmina`, para manter os lados ativos juntos.
1. Como pessoa diagramadora, quero que uma Página única gere somente sua área ativa, para não produzir espaço para o lado inativo.
1. Como pessoa diagramadora, quero exportar `Por página`, para receber cada lado ativo separadamente.
1. Como pessoa diagramadora, quero recortar no centro um Frame atravessado durante a Exportação `Por página`, para separar corretamente os dois lados.
1. Como pessoa diagramadora, quero numerar somente Páginas ativas, para evitar lacunas causadas por lados inativos.
1. Como pessoa diagramadora, quero nomear JPEGs e PNGs como `{nome-do-projeto}_{índice de três dígitos}`, para obter nomes previsíveis.
1. Como pessoa diagramadora, quero usar a posição da Lâmina ou a Numeração de Página como índice conforme o modo, para relacionar cada arquivo ao Álbum.
1. Como pessoa diagramadora, quero que os modos `Por lâmina` e `Por página` compartilhem o mesmo namespace de nomes, para manter a convenção simples mesmo quando o modo não puder ser inferido pelo arquivo.
1. Como pessoa diagramadora, quero preservar índices globais em uma seleção parcial, para conservar a posição original.
1. Como pessoa diagramadora, quero gerar um único `{nome-do-projeto}.pdf` multipágina, para receber a seleção consolidada.
1. Como pessoa diagramadora, quero que cada página do PDF represente a unidade do modo escolhido, para manter a mesma semântica.
1. Como pessoa diagramadora, quero usar por padrão uma pasta com o Nome do Projeto ao lado de seu arquivo, para localizar facilmente a saída.
1. Como pessoa diagramadora, quero escolher outro destino, para integrar a saída ao meu fluxo de entrega.
1. Como pessoa diagramadora, quero exportar toda a Dimensão configurada, inclusive a Sangria, para produzir o arquivo físico correto.
1. Como pessoa diagramadora, quero excluir as linhas-guia da saída, para não renderizar elementos de interface.
1. Como pessoa diagramadora, quero ver todos os conflitos antes do início, para decidir globalmente entre sobrescrever ou cancelar.
1. Como pessoa diagramadora, quero ver bloqueios de Exportação em uma tabela com Projeto, problema e ação, para corrigi-los no contexto adequado.
1. Como pessoa diagramadora, quero abrir o Projeto quando houver placeholder ou relinkar a pasta de Fotos quando faltar um original, para resolver cada tipo de problema pelo fluxo correto.
1. Como pessoa diagramadora, quero impedir substituição ou renomeação silenciosa, para conservar controle sobre os arquivos.
1. Como pessoa diagramadora, quero remover saídas excedentes de uma Exportação integral anterior, para que a pasta reflita a quantidade atual do Álbum.
1. Como pessoa diagramadora, quero detectar órfãos apenas pelo Nome exato, índice e extensão atual, para limitar a limpeza à convenção conhecida.
1. Como pessoa diagramadora, quero impedir a limpeza de Saídas órfãs quando a Publicação falhar, para que uma tentativa incompleta não apague as saídas excedentes anteriores; arquivos já promovidos não são revertidos.
1. Como pessoa diagramadora, quero que todas as saídas sejam renderizadas e verificadas em preparação no volume de destino antes da publicação, para evitar publicar arquivos ainda incompletos.
1. Como pessoa diagramadora, quero que cada arquivo final seja promovido atomicamente quando o destino suportar e que uma falha de publicação informe claramente a possível mistura entre saídas antigas e novas, para não receber uma promessa irreal de rollback.
1. Como pessoa diagramadora, quero decidir quando tentar novamente após uma falha, para que uma ação final nunca seja repetida automaticamente.
1. Como pessoa diagramadora, quero que uma Exportação parcial nunca remova arquivos fora da seleção, para não apagar saídas válidas.
1. Como pessoa diagramadora, quero ser avisada de que uma Exportação parcial não consegue provar o modo das saídas preexistentes sem manifesto, para não confundir um intervalo atualizado com um conjunto integral coerente.
1. Como pessoa diagramadora, quero manter arquivos com outro Nome ou extensão, para restringir a limpeza ao conjunto correspondente.
1. Como pessoa diagramadora, quero evitar manifesto ou arquivo auxiliar, para manter a pasta contendo somente as saídas finais.

### Operações em lote

1. Como operadora de lote, quero usar o estado atual visível de um Projeto como modelo, inclusive mudanças não salvas, para gerar trabalhos conforme a composição que vejo.
1. Como operadora de lote, quero que a geração não salve nem modifique o modelo, para preservar meu controle sobre ele.
1. Como operadora de lote, quero selecionar uma árvore de origem e um destino, para gerar vários Projetos em uma operação.
1. Como operadora de lote, quero que cada pasta com imagem suportada diretamente gere um Projeto com seu nome, para transformar a organização das Fotos em trabalhos.
1. Como operadora de lote, quero continuar buscando em subpastas mesmo quando a pasta atual gerar um Projeto, para processar toda a hierarquia.
1. Como operadora de lote, quero ignorar pastas com somente formatos não suportados, para não criar Projetos sem imagens reconhecidas.
1. Como operadora de lote, quero reproduzir a estrutura relativa no destino, para conservar agrupamentos da origem.
1. Como operadora de lote, quero que cada resultado seja uma cópia completa e independente do modelo, para preservar Lâminas, composição, padrões e favoritos.
1. Como operadora de lote, quero acrescentar as novas imagens somente à aba `Fotos`, para não classificá-las como Decorativos.
1. Como operadora de lote, quero manter essas imagens apenas no Painel, para decidir sua composição posteriormente.
1. Como operadora de lote, quero vincular os originais sem copiá-los, para evitar duplicação de mídia.
1. Como operadora de lote, quero bloquear destino igual ou interno à origem, para impedir que resultados entrem na própria varredura.
1. Como operadora de lote, quero conhecer todos os conflitos antes do início, para escolher sobrescrever, ignorar ou cancelar.
1. Como operadora de lote, quero proteger um Projeto de destino aberto contra sobrescrita, para preservar sua sessão ativa.
1. Como operadora de lote, quero que falhas individuais não interrompam nem revertam os demais, para concluir todo o trabalho possível.
1. Como operadora de lote, quero receber resumo de criados, ignorados e falhas, para conferir o resultado.
1. Como operadora de lote, quero localizar Projetos recursivamente para Exportação, para processar uma árvore completa.
1. Como operadora de lote, quero exportar sempre o Álbum inteiro, para manter o comportamento previsível.
1. Como operadora de lote, quero escolher formato e modo uma vez, para aplicar configurações uniformes.
1. Como operadora de lote, quero usar somente o estado salvo de cada arquivo, para não depender de sessões abertas imprevisíveis.
1. Como operadora de lote, quero que mudanças não salvas sejam ignoradas sem Salvamento automático, para manter o persistido como única fonte.
1. Como operadora de lote, quero que a Exportação em lote assuma controle exclusivo do aplicativo, para dedicar os recursos disponíveis ao processamento.
1. Como operadora de lote, quero que somente a janela de progresso e cancelamento permaneça interativa, para acompanhar uma única operação global sem disputar com as Janelas de Projeto.
1. Como operadora de lote, quero que Cache e demais trabalhos em segundo plano sejam pausados durante o lote, para concentrar CPU e memória na saída final.
1. Como operadora de lote, quero que meus Projetos abertos e suas alterações não salvas permaneçam intactos ao final, para que o bloqueio temporário não modifique meu trabalho.
1. Como operadora de lote, quero usar por padrão a pasta de cada Projeto ou escolher uma raiz alternativa, para adequar a organização da saída.
1. Como operadora de lote, quero preservar a hierarquia relativa e criar uma pasta por Projeto no destino alternativo, para não misturar saídas.
1. Como operadora de lote, quero levantar conflitos de todo o lote antes do início, para escolher entre sobrescrever todos ou cancelar.
1. Como operadora de lote, quero relinkar temporariamente todos os originais encontrados em pastas correspondentes aos Nomes dos Projetos, para concluir o lote sem regravar silenciosamente os arquivos.
1. Como operadora de lote, quero ignorar um Projeto inválido e continuar os demais, para concluir todos os Álbuns válidos.
1. Como operadora de lote, quero processar um Projeto por vez na primeira versão, para manter coordenação e recuperação previsíveis antes de medir a necessidade de paralelismo.
1. Como operadora de lote, quero escolher entre retomar ou encerrar um lote interrompido pela queda do programa, para nunca reiniciar uma ação final silenciosamente.
1. Como operadora de lote, quero que a retomada preserve itens concluídos e refaça integralmente o item interrompido, para nunca continuar no meio de uma publicação.
1. Como operadora de lote, quero que encerrar a recuperação mantenha as Exportações concluídas e descarte somente preparações incompletas, para não perder resultados válidos.
1. Como operadora de lote, quero limpar órfãos isoladamente após o sucesso de cada Projeto, para que falhas externas não afetem sua pasta.
1. Como operadora de lote, quero receber resumo de exportados, ignorados e falhas com motivos, para auditar a operação completa.

## Implementation Decisions

Esta SPEC é a fonte normativa do comportamento funcional do produto. ADRs possuem decisões arquiteturais difíceis de reverter; `CONTEXT.md` possui somente o significado dos termos; cada documento de design possui o contrato detalhado de sua área; tickets possuem o escopo executável e critérios derivados dessas fontes, mas permanecem autocontidos e declaram suas fontes normativas.

Quando duas fontes parecerem incompatíveis, a implementação deve parar até que o documento proprietário da decisão seja reconciliado. A proximidade de um ticket com a implementação não lhe permite substituir silenciosamente SPEC, ADR, glossário ou design.

### Tela de Boas-vindas

- A janela visível do processo principal `MyAlbuns.exe` é uma Tela de Boas-vindas separada das Janelas de Projeto e não contém Canvas ou estado criativo.
- A primeira versão oferece seis entradas principais: `Novo Projeto`, `Abrir Projeto`, `Projetos recentes`, `Exportação em lote`, `Configurações` e `Ajuda`.
- `Projetos recentes` ocupa a região principal e mais ampla da janela.
- `Novo Projeto`, `Abrir Projeto` e `Exportação em lote` formam um grupo de ações principais ao lado dos recentes; `Configurações` e `Ajuda` aparecem como ações secundárias na região inferior.
- Cada Projeto recente é uma linha textual com o Nome do Projeto em destaque e o caminho completo abaixo, em tamanho menor; a primeira versão não mostra miniatura ou data.
- Clicar em qualquer ponto do item solicita a abertura do Projeto correspondente.
- A lista é ordenada pela abertura mais recente; abrir um Projeto por qualquer fluxo move sua entrada imediatamente para o topo.
- `Novo Projeto` inicia o fluxo de criação; `Abrir Projeto` usa o diálogo do sistema operacional; `Projetos recentes` permite reabrir trabalhos conhecidos pelo aplicativo.
- `Exportação em lote` pode começar na Tela de Boas-vindas porque encontra Projetos persistidos em uma pasta e não depende do estado de uma sessão aberta.
- A Geração de Projetos em lote permanece exclusivamente na Janela do Projeto usado como modelo e não aparece na Tela de Boas-vindas.
- Abrir o primeiro Projeto oculta a Tela de Boas-vindas sem encerrar o `MyAlbuns.exe`; fechada a última Janela de Projeto, a Tela reaparece.

### Configurações do aplicativo

- `Configurações` abre uma janela global organizada nas abas iniciais `Desempenho` e `Photoshop`.
- A janela pode ser aberta pela Tela de Boas-vindas ou por `Ferramentas > Configurações` em qualquer Janela de Projeto.
- Existe somente uma janela de Configurações por instância do MyAlbuns; solicitações posteriores focalizam a existente.
- As preferências dessa janela pertencem ao usuário e não integram arquivos de Projeto, Salvamento ou Undo/Redo.
- Escolhas simples são aplicadas e persistidas imediatamente, sem botão geral `Aplicar` ou `Salvar`. `Liberar espaço` e `Limpar todo o Cache` mantêm seus próprios feedbacks e confirmações.
- Preferências globais, catálogo global de Layouts e estado local reconstruível são persistidos independentemente, cada qual com schema e política de falha próprios. Podem compartilhar primitivas de substituição de um arquivo, mas não um armazenamento genérico que iguale suas garantias. Cada Janela consulta a revisão aplicável ao abrir, receber foco ou solicitar atualização manual; falha de escrita preserva a revisão confirmada anterior.
- `Desempenho` apresenta somente o uso do Cache, o volume liberável de Projetos fechados, `Liberar espaço` e `Limpar todo o Cache`; não expõe calibração, processos, threads, memória ou paralelismo.
- Apagar o Cache não remove itens do Painel, vínculos, conteúdo de Projeto ou Arquivos originais. As representações interativas são reconstruídas quando necessárias e continuam proibidas como fonte de Exportação.
- A seção mostra o espaço atualmente ocupado e quanto `Liberar espaço` pode remover.
- `Liberar espaço` exige confirmação e remove integralmente somente o Cache de Projetos fechados; qualquer Projeto aberto ou com processo ativo é preservado.
- O aplicativo não possui limite rígido nem apaga Cache automaticamente por tamanho na primeira versão.
- O aplicativo não possui alertas progressivos por patamares arbitrários. Pouco espaço livre no volume pode gerar um aviso com o total ocupado, `Agora não` e `Liberar espaço`; se não houver espaço seguro para outro artefato, a geração daquele Cache é interrompida sem modificar o Projeto ou o original.
- `Limpar todo o Cache` exige confirmação e só executa imediatamente quando não houver Projeto ou Processador ativo. Caso contrário, oferece agendar a limpeza para a próxima inicialização, antes da abertura de Projetos; o MVP não pausa editores nem remove Cache ativo ao vivo.
- `Photoshop` lista as instalações compatíveis detectadas e mostra a disponibilidade da integração.
- Sem preferência válida, a versão mais recente é selecionada automaticamente. O usuário pode escolher outra versão detectada ou usar `Localizar Photoshop...` para indicar manualmente o executável.
- A instalação escolhida é uma preferência global reutilizada entre Projetos e sessões.
- `Abrir no Photoshop` aparece no menu de contexto de uma Foto no Painel e de um Frame preenchido. `Ctrl + E` é seu atalho fixo na primeira versão.
- O comando exige exatamente uma Foto contextual. Com Seleção de Frames múltipla, ele fica indisponível e não abre arquivos em massa.
- A integração envia sempre o Arquivo vinculado original, nunca o Cache, o recorte do Frame ou uma versão com os ajustes não destrutivos do MyAlbuns incorporados.
- A ausência do Photoshop desabilita somente a integração e não impede edição, Salvamento ou Exportação no MyAlbuns.
- Se a instalação escolhida deixar de existir ou não puder ser iniciada, a ação falha sem modificar o Projeto e sem recorrer ao Cache; o aviso oferece acesso à configuração para escolher ou localizar outra instalação.
- Atalhos e modificadores são fixos e visíveis no MVP. Seus identificadores, descrições, contextos e associações padrão permanecem estáveis para menus e dicas; foco, seleção, reconhecimento de gestos e acionamento permanecem no contexto da interface que os possui.
- Comandos de domínio, comandos de aplicação e ações somente da interface não são tratados como uma categoria única no Histórico. A interface de remapeamento fica adiada e, quando priorizada, reutiliza os identificadores estáveis começando pelos atalhos de teclado.
- `Esc` permanece reservado para cancelar a operação atual ou sair de um modo.

### Criação de Projeto

- `Novo Projeto` abre um fluxo do próprio aplicativo composto por exatamente duas etapas: `Dimensões` e `Personalização`.
- `Dimensões` contém Unidade, largura e altura da Lâmina, DPI, quantidade inicial de Lâminas, formato da primeira e da última Lâmina, Sangria e Área de segurança.
- A etapa organiza esses campos nos grupos `Documento`, `Estrutura` e `Áreas técnicas`, respectivamente.
- Não há reprodução gráfica nessa etapa. Um resumo somente de leitura mostra continuamente a dimensão da Lâmina, a dimensão calculada de cada Página e o DPI.
- `Próximo` valida todos os campos de `Dimensões` e só avança quando o conjunto estiver válido.
- Erros aparecem junto aos respectivos campos e o foco vai para o primeiro inválido; o fluxo não abre um modal genérico de validação.
- `Personalização` contém Background padrão, Overlay padrão e Padrão dos Frames.
- Background e Overlay permitem `Escolher imagem...` pelo seletor nativo do Windows. A seleção permanece provisória e só é vinculada à aba `Decorativos` depois que a criação for concluída com sucesso.
- Cancelar o fluxo não importa ou copia imagens provisórias nem altera seus arquivos originais.
- `Personalização` apresenta uma reprodução viva de uma Lâmina com Frames de demonstração, atualizada imediatamente para mostrar Background, Overlay e presença, cor e espessura da Borda padrão.
- A reprodução respeita a proporção de largura e altura escolhida em `Dimensões`, mas sempre representa uma Lâmina dupla, independentemente do formato das extremidades, para expor esquerda, direita e Ambos os lados na mesma superfície.
- Essa reprodução reutiliza o seletor espacial de `Design do Álbum`: hover realça esquerda, direita ou ambos pela região central; o clique fixa o escopo usado pelos controles de Background e Overlay.
- O hover sem clique é apenas feedback temporário e não muda o escopo configurado.
- A reprodução aceita as imagens provisórias escolhidas, mas permanece somente visual e não cria Lâmina, Frame ou Foto no Projeto resultante.
- `Personalização` usa duas colunas: reprodução ampla à esquerda e controles de Background, Overlay e Padrão dos Frames à direita.
- `Voltar`, `Cancelar` e `Criar` permanecem em um rodapé fixo; se os controles precisarem rolar, a reprodução e o rodapé continuam visíveis.
- O Nome e a Localização do arquivo não aparecem nessas etapas. Somente `Criar`, na etapa final, abre o diálogo nativo do Windows para escolhê-los.
- Cancelar o diálogo nativo não cria arquivo e retorna ao fluxo do aplicativo preservando todos os valores preenchidos.

### Estrutura da interface

- Toda operação que precisa de uma janela de progresso usa a mesma representação minimalista.
- Com total conhecido, ela mostra somente uma barra geral e `X/Y`; sem total confiável, usa barra animada indeterminada e omite a contagem.
- `Cancelar` é a única ação opcional e só aparece quando a operação suporta interrupção segura; operações não canceláveis não mostram esse botão.
- A janela não apresenta tabela por Projeto, múltiplas barras, lista de trabalhos simultâneos ou contagens separadas por status.
- A janela de progresso nunca se transforma em resumo: sucesso integral a fecha e mostra uma confirmação curta; ignorados ou falhas abrem a Tela de Problemas com Projeto, Resultado e ações.
- A Janela do Projeto possui uma barra de menus superior com os grupos iniciais `Arquivo`, `Editar`, `Lâmina`, `Exibir`, `Ferramentas` e `Ajuda`.
- `Lâmina` oferece `Adicionar antes`, `Adicionar depois`, `Duplicar Lâmina`, `Excluir` e `Converter extremidade`, usando a Lâmina mais centralizada como alvo.
- Os mesmos comandos aparecem no menu de contexto da superfície ou Barra de uma Lâmina e usam o item clicado como alvo explícito. A conversão só é habilitada em uma extremidade válida.
- Durante o Modo de edição, todos os comandos que adicionam, duplicam, excluem, convertem ou reordenam Lâminas ficam desabilitados; a sequência só pode ser alterada depois de sair com `Esc`.
- Abaixo do menu, a interface se divide em uma coluna de trabalho à esquerda e um Painel contextual fixo à direita.
- A coluna de trabalho contém o Canvas contínuo na parte superior e o Painel de imagens na parte inferior. O Painel de imagens não avança sob o Painel contextual.
- Um splitter horizontal redimensiona Canvas e Painel de imagens. `Exibir > Painel de imagens` oculta ou restaura o Painel; ocultá-lo entrega a altura disponível ao Canvas.
- Altura e visibilidade do Painel de imagens são preferências da interface lembradas entre sessões, sem alterar o Projeto ou Undo/Redo.
- Um splitter vertical separa a coluna de trabalho inteira do Painel contextual e permite ajustar a largura direita sem sobreposição.
- `Exibir > Painel contextual` oculta ou restaura essa região. Ocultá-lo entrega toda a largura à coluna de trabalho; largura e visibilidade são preferências lembradas entre sessões, sem alterar o Projeto ou Undo/Redo.
- O Canvas apresenta as Lâminas lado a lado em uma sequência horizontal contínua. Não existe um navegador lateral independente.
- O modelo lógico mantém todas as Lâminas do Álbum e não impõe um máximo arbitrário. A cena detalhada e suas texturas são materializadas somente para a área visível e uma margem de pré-carga adjacente.
- Ao sair dessa faixa, uma Lâmina conserva seu estado lógico, mas pode liberar recursos gráficos pesados; retornar à faixa reconstrói sua representação sem alterar o Projeto. A política concreta de residência e descarte será calibrada por testes de estresse com Álbuns longos.
- No modo normal não existe uma Lâmina ativa exclusiva: todas as Lâminas apresentadas são interativas.
- Cada Lâmina possui uma Barra própria imediatamente acima dela. A Barra pertence somente à interface, acompanha a largura visual da Lâmina e nunca participa da Exportação.
- A Barra mostra a Numeração de Página sobre cada lado ativo e o número da Lâmina alinhado à direita.
- O controle de duas setas troca os Frames entre as Páginas esquerda e direita, levando consigo as Fotos contidas, sem trocar os números das Páginas.
- A Troca de lados translada cada Frame totalmente contido em uma Página para a mesma posição relativa da Página oposta. Não espelha Frame ou Foto e preserva dimensões, Pan, Zoom, estilo e ordem visual.
- Frames com Travessia central permanecem inalterados. A ação fica indisponível em Página única e em Layout travado e constitui uma única ação de Undo/Redo.
- O controle central em forma de grade abre ou fecha o Painel de Layouts usando aquela Lâmina como alvo explícito, sem depender da Lâmina centralizada nem alterar a interatividade das demais.
- Existe somente um Painel de Layouts aberto por vez. Ele ocupa horizontalmente a coluna de trabalho acima do Canvas, termina antes do Painel contextual e desloca as Lâminas para baixo sem cobri-las.
- Clicar novamente no controle da mesma Lâmina fecha o Painel; clicar no controle de outra reutiliza a faixa e troca seu alvo. Layouts não aparecem no Painel contextual direito.
- A Barra da Lâmina e o Painel de Layouts existem somente no Canvas contínuo do modo normal; ambos ficam ausentes e indisponíveis no Modo de edição.
- Se o Painel estava aberto ao entrar no Modo de edição, seu alvo e estado ficam suspensos; ao sair com `Esc`, ele reaparece para a mesma Lâmina com candidatos recalculados. Um Painel anteriormente fechado continua fechado.
- O Canvas contínuo do modo normal não possui Zoom. O Zoom de visualização existe exclusivamente para a Lâmina isolada no Modo de edição.
- No modo normal, todas as Lâminas compartilham uma escala automática que enquadra sua altura completa com margem; não existe rolagem vertical, somente navegação horizontal.
- Redimensionar a Janela ou o splitter do Painel de imagens recalcula essa escala sem alterar o Projeto ou criar um estado de Zoom.
- Fora do Modo de edição, `Alt` + clique e arraste sobre um Frame faz Pan da Foto e `Alt` + roda do mouse altera o Zoom da Foto sob o ponteiro, sem mudar a geometria do Frame; ambos integram a `MediaTransform` persistente da colocação.
- `Alt` + pressionar e iniciar o arraste é consumido como Pan e não seleciona o Frame; `Alt` + roda também preserva a seleção atual.
- O arraste completo gera uma ação ao soltar; eventos consecutivos de `Alt` + roda são agrupados em uma ação quando a sequência termina.
- Soltar uma Foto usa a Lâmina sob o ponteiro como destino. Selecionar um Frame ou Foto em outra Lâmina transfere a seleção diretamente para aquele elemento.
- Clicar em uma área vazia limpa a seleção do elemento e retorna ao contexto do Álbum. Clicar na Grade de Lâminas ou usar as setas navega; arrastar uma miniatura da Grade reordena; rolar o Canvas não altera o contexto.
- A Lâmina centralizada no Canvas é aquela cujo centro visual está mais próximo do centro horizontal da área visível. É apenas um alvo implícito transitório e não torna as demais Lâminas inativas.
- O Modo de edição da Lâmina oculta temporariamente as demais, centraliza e amplia a escolhida e reduz o Painel de imagens para aumentar o Canvas.
- Não existe faixa, rótulo, botão de retorno ou mudança adicional de fundo: o isolamento da única Lâmina e a nova proporção das regiões identificam o modo; `Esc` continua sendo a saída.
- Cada entrada no Modo de edição inicia em `Ajustar Lâmina`, na maior escala que mantém a Lâmina inteira visível. Sair descarta o Zoom e uma nova entrada volta ao ajuste inicial.
- Zoom e Pan do Canvas formam a `ViewportTransform`, estado temporário da interface sem persistência no Projeto, participação em Undo/Redo ou presença no `RenderSnapshot`.
- O Zoom de edição é alterado por `Ctrl` + `+`, `Ctrl` + `−` e `Ctrl` + roda do mouse; `Ctrl` + `0` retorna para `Ajustar Lâmina`. Não há slider, botões ou percentual permanente.
- `Ctrl` + roda ancora no ponto sob o cursor; `Ctrl` + `+` e `Ctrl` + `−` ancoram no centro visível do Canvas.
- Acima do ajuste inicial, `Espaço` + arraste com o botão esquerdo ou arraste com o botão do meio fazem Pan do Canvas, com cursor de mão e sem modificar conteúdo.
- `Ajustar Lâmina` é o limite mínimo do Zoom; o limite máximo é calibrado no protótipo conforme nitidez e responsividade.
- No Modo de edição, gestos diretos selecionam um ou vários Frames e permitem suas operações geométricas; os atalhos diretos de Pan e Zoom da Foto não atuam nesse modo.
- Nesse modo, o Painel de imagens assume uma altura compacta sem desaparecer. Ao sair, o Canvas contínuo retorna à posição anterior e restaura a altura normal do Painel.
- Dois cliques numa Lâmina entram no Modo de edição para ela. Com foco no Canvas, `Enter` entra para a Lâmina centralizada e `Esc` retorna ao Canvas contínuo.
- Dois cliques em uma Foto no Painel de imagens usam a Lâmina centralizada ou, no Modo de edição, a Lâmina isolada. Se houver placeholders, preenchem primeiro o mais à esquerda; sem placeholder, criam um novo Frame conforme as regras do modo.
- O placeholder do duplo clique é ordenado pela coordenada horizontal da borda esquerda e, em empate, pela coordenada vertical da borda superior, ambas crescentes.
- Em Layout travado sem placeholder, o duplo clique é recusado com orientação para arrastar a Foto até um placeholder disponível.
- Arrastar uma Foto sobre um Frame preenchido ou placeholder usa somente esse Frame como alvo, substituindo ou preenchendo sua Foto sem propagar a ação para a Seleção de Frames.
- Em Frames sobrepostos, o alvo da Foto é sempre o mais acima na Pilha visual cujo retângulo externo contém o ponteiro, independentemente de conteúdo, transparência ou Opacidade; não existe gesto para alternar para um Frame inferior.
- No Modo de edição, soltar uma Foto em área vazia cria um Frame com a proporção padrão da Criação manual de Frame, centralizado no ponto de soltura e deslocado integralmente para dentro da superfície ativa quando necessário, sem reduzir seu tamanho.
- No modo normal, a soltura vazia escolhe somente a Lâmina e o primeiro Layout compatível define a nova geometria. Em Layout travado, área vazia não é alvo válido.
- Durante o arraste de Foto, somente o Frame superior atingido recebe destaque de alvo; em área vazia válida, somente a Lâmina é destacada.
- Não existe prévia composta da Foto, do novo Frame ou do Layout resultante. Alvo inválido apresenta feedback de bloqueio.
- `Esc` e soltura fora de alvo válido cancelam sem mutação ou Histórico; somente a soltura válida executa a operação.
- Inserir, preencher ou substituir uma Foto deixa somente o Frame afetado selecionado e atualiza o Painel contextual. A troca de seleção é estado da interface e não cria ação separada de Undo/Redo.
- Dois cliques em um Decorativo aplicam-no como Background a Ambos os lados da Lâmina centralizada; `Shift` + dois cliques aplicam-no como Overlay. No Modo de edição, Ambos os lados da Lâmina isolada são o destino. A aplicação a somente um lado exige arraste explícito.
- O Painel contextual tem rolagem própria e organiza ferramentas em seções recolhíveis.
- Estados abertos/fechados são lembrados separadamente nos contextos do Álbum, de `Design da Lâmina` e de Frame/Foto e restaurados ao retornar.
- A preferência de expansão é reutilizada entre Projetos e sessões, sem alterar o Projeto ou Undo/Redo.
- Sem Frame ou Foto selecionada, o contexto mostra `Informações do Álbum`, `Design do Álbum` e `Grade de Lâminas`.
- Na Grade, clique sem arraste centraliza a Lâmina. Arrastar além do limiar cria espaço reservado, fantasma e deslocamento das células intermediárias segundo a sequência, usando as mesmas validações, cancelamento e Undo/Redo do arraste pela Barra.
- Durante esse arraste, aproximar o fantasma das bordas superior ou inferior rola verticalmente o contêiner visível da Grade, com velocidade progressiva e atualização contínua do espaço reservado.
- Somente a superfície que iniciou o arraste mostra a prévia de reordenação. Canvas ou Grade oposto mantém a ordem confirmada e sincroniza de uma vez apenas depois da soltura válida.
- `Informações do Álbum` é somente de leitura e resume quantidade de Lâminas e Páginas, dimensões da Lâmina e da Página, Unidade, DPI, formatos das extremidades, Sangria, segurança e contagens de Frames placeholder ou Arquivos originais ausentes, distinguindo os ausentes em uso.
- Placeholders e originais ausentes em uso são destacados como bloqueios de Exportação; ausentes não usados são apenas aviso. Todas as alterações globais permanecem em `Design do Álbum`.
- Clicar em placeholders expande a Grade de Lâminas destacando as afetadas. Clicar em originais ausentes abre o Painel de imagens no filtro `Ausentes`, com badges de quantidade nas abas `Fotos` e `Decorativos`.
- `Ausentes` aberto pelo aviso é uma visualização temporária: guarda aba e filtros anteriores, restaura-os ao ser encerrada e não é persistida como preferência entre sessões.
- `Design do Álbum` começa organizado em `Estrutura` (configuração das extremidades), `Documento` (Unidade, dimensões da Lâmina e DPI), `Áreas técnicas` (Sangria e segurança), `Padrões visuais` (Background e Overlay) e `Padrão dos Frames` (presença, cor e espessura da Borda).
- `Quantidade de Lâminas` existe somente no diálogo de criação. Depois disso, a quantidade muda apenas como consequência dos comandos explícitos de adicionar ou excluir Lâminas, nunca por um campo global em `Design do Álbum`.
- `Estrutura` contém somente `Primeira Lâmina` e `Última Lâmina`, cada uma com as opções independentes `Lâmina dupla` e `Página única`. `Aplicar` apresenta o impacto antes de confirmar as conversões.
- Em `Documento`, trocar Unidade converte imediatamente somente os valores exibidos. Largura, altura e DPI permanecem pendentes até um único `Aplicar`, que apresenta tamanho físico e resolução finais e confirma tudo atomicamente em uma ação de Undo/Redo.
- A transformação de largura e altura respeita os limites dimensionais seguros; a mudança de DPI recalcula a resolução em pixels das representações derivadas e da Exportação sem, isoladamente, alterar geometria, Pan ou enquadramentos.
- `Áreas técnicas` apresenta Sangria e segurança na Unidade do Projeto. `Enter` ou a saída do campo confirma cada valor válido, atualiza máscara e guias e cria uma ação de Undo/Redo; valor inválido mostra erro inline e não é aplicado.
- `Áreas técnicas` não possui `Aplicar` ou confirmação modal, pois suas mudanças não redimensionam nem reorganizam a composição.
- `Padrões visuais` começa com uma miniatura do padrão global, não de uma Lâmina específica. Hover e clique selecionam esquerda, direita ou Ambos os lados pela região central; Background e Overlay abaixo atuam no escopo escolhido.
- Clicar no preview de Background ou Overlay abre um seletor compacto com somente os Decorativos já importados no Projeto. Escolher um item altera o padrão, atualiza as aplicações herdadas e cria uma ação de Undo/Redo.
- Esse seletor não aceita arraste nem importa arquivos; novos Decorativos continuam sendo importados exclusivamente pelo Painel de imagens.
- `Padrão dos Frames` mostra uma prévia simples, `Exibir borda`, cor e espessura na Unidade do Projeto. Cada alteração é uma ação imediata de Undo/Redo e atualiza os Frames que usam o design do Álbum.
- Opacidade não integra o padrão global e permanece disponível somente no contexto individual do Frame.
- Configurações globais futuras entram no grupo correspondente ou justificam um novo grupo; ajustes exclusivos de uma Lâmina ou elemento permanecem fora de `Design do Álbum`.
- `Design do Álbum` não possui um botão geral de salvamento. Mudanças simples são aplicadas imediatamente ao estado aberto e entram em Undo/Redo; mudanças estruturais ou dimensionais possuem `Aplicar` próprio, validação e confirmação do impacto.
- Aplicar uma configuração nunca persiste o arquivo automaticamente. O Projeto continua com alterações pendentes até o comando manual `Salvar`.
- No Modo de edição, a ausência de seleção de Frame ou Foto troca esse conteúdo por `Design da Lâmina` no Painel contextual direito.
- `Design da Lâmina` contém uma miniatura real da composição corrente: hover à esquerda ou à direita realça somente o respectivo lado; hover na região central realça ambos os lados; clicar fixa visualmente o escopo selecionado sem alterar a composição.
- A miniatura é atualizada conforme a Lâmina muda. Realces de hover e seleção são sobreposições exclusivas da interface e não pertencem ao Projeto nem à Exportação.
- Em Página única, a miniatura mostra o lado desativado como área neutra e totalmente inerte. Somente a Página ativa responde a hover e clique, e a região central não seleciona ambos os lados.
- Ao entrar no Modo de edição, a seleção inicial é o escopo `Ambos os lados` para uma Lâmina dupla e a Página ativa para uma Página única.
- Alternar temporariamente para o contexto de Frame/Foto e retornar a `Design da Lâmina` restaura o último escopo escolhido durante aquela sessão de edição. Sair do modo descarta a seleção.
- A seleção da miniatura é estado transitório da interface, sem Salvamento, Undo/Redo ou persistência no Projeto.
- Os controles de Background e Overlay ficam abaixo dessa representação e atuam sobre o escopo selecionado, incluindo ações para remover a aplicação local ou voltar a usar o design do Álbum.
- `Design da Lâmina` contém o botão `Salvar disposição como Layout`; o mesmo comando existe no menu `Editar` e continua disponível durante todo o Modo de edição, mesmo quando o contexto direito mostra Frame/Foto.
- A representação de escopo pertence somente ao Painel contextual e não cria controles ou zonas permanentes sobre o Canvas.
- A miniatura e os blocos de Background/Overlay não aceitam Decorativos arrastados. Imagens decorativas são aplicadas exclusivamente pelos gestos já definidos no Canvas, sem duplicar esse fluxo no Painel contextual.
- Para o escopo selecionado, `Background` mostra preview da cor ou imagem atual, seletor de cor, `Remover` e `Voltar ao design do álbum`; `Overlay` mostra preview da imagem ou `Sem overlay`, `Remover` e a mesma ação de retorno.
- Escolher uma cor cria uma definição local de Background no escopo selecionado e uma única ação de Undo/Redo. Substituições por imagens continuam sendo realizadas no Canvas.
- Cada papel mostra sempre `Usando o design do álbum` ou `Definido nesta lâmina`. `Voltar ao design do álbum` só aparece para uma definição local; `Remover` permanece disponível e torna somente o Background selecionado branco ou o Overlay selecionado ausente.
- Se o escopo `Ambos os lados` estiver selecionado enquanto esquerda e direita tiverem valores diferentes, cada papel mostra previews `Esquerda` e `Direita` com suas respectivas origens amigáveis.
- Uma ação nessa seleção afeta ambos os lados como uma única operação de Undo/Redo. Alterar somente um lado exige selecioná-lo na miniatura.
- Com Frame ou Foto selecionada, o contexto mostra preview e informações da imagem, `Design` e `Ajustes e Efeitos`.
- `default` e `custom` são estados internos e nunca aparecem literalmente na interface. O Painel mostra `Usando o design do álbum` para valores herdados, `Definido nesta lâmina` para definições locais e a ação `Voltar ao design do álbum` para restaurar a herança.
- `Design` oferece controles para Zoom, Ângulo, Opacidade e Borda. Ângulo varia de `-45°` a `+45°` e é independente do Giro de 90° anti-horário.
- Ângulo usa slider e campo numérico com precisão de `0,1°`. Dois cliques no slider restauram `0°`.
- Sliders não possuem botão próprio de restauração. Dois cliques restauram o valor padrão da respectiva propriedade; quando o controle edita o Projeto, o reset gera uma única ação de Undo/Redo. Sliders operacionais, como Qualidade JPEG, restauram seu próprio padrão sem alterar o Histórico.
- Na primeira versão, `Ajustes e Efeitos` contém somente `Preto e branco`. Brilho, contraste, saturação e filtros adicionais não aparecem desabilitados ou como placeholders e só entram quando forem implementados.

### Estrutura física e editorial

- Cada Projeto representa exatamente um Álbum independente, formado por ao menos duas Lâminas ordenadas.
- O papel de uma Lâmina como inicial, interna ou final deriva de sua posição na sequência.
- Uma Lâmina possui lados esquerdo e direito; somente lados ativos representam Páginas. Um lado inativo não contém conteúdo e não permite interação.
- As Lâminas inicial e final podem ser duplas ou de página única. Na inicial simples, somente o lado direito fica ativo; na final simples, somente o esquerdo.
- Uma Página única permanece presa à extremidade correspondente. Reordenações não podem deslocá-la indiretamente para outra posição.
- Pressionar uma área não interativa da Barra e ultrapassar o limiar de arraste inicia a reordenação; os controles da Barra não iniciam o gesto.
- A origem mantém um espaço reservado do mesmo tamanho e um fantasma da Lâmina acompanha o ponteiro.
- Ao alcançar outra posição válida, o espaço reservado se desloca até ela e as Lâminas intermediárias avançam uma posição em direção à origem. Trata-se de inserção, não de troca direta.
- Aproximar o fantasma de uma borda lateral inicia rolagem horizontal automática na direção correspondente; a velocidade aumenta com a proximidade da borda e o espaço reservado continua sendo atualizado.
- Durante a prévia, a ordem do Projeto permanece intacta. Soltar em posição válida confirma uma única ação de Undo/Redo; `Esc` ou soltura inválida restaura a ordem original.
- A representação que não originou o gesto não anima nem muda durante a prévia; ela recebe a nova ordem somente no commit válido.
- Posições que empurrariam uma Página única para o interior não recebem o espaço reservado. Após confirmar, papéis e Numeração são recalculados e a Lâmina movida é centralizada fora do Histórico.
- `Adicionar antes` e `Adicionar depois` criam uma Lâmina dupla vazia, sem Frames, Fotos ou Layout, cujos Background e Overlay começam herdando os padrões atuais do Projeto.
- Inserir para fora de uma extremidade dupla é permitido e faz a nova Lâmina assumir o papel correspondente. O comando fica desabilitado se empurraria uma extremidade de Página única para o interior.
- Depois da inserção, a Numeração de Página é recalculada e o Canvas centraliza a nova Lâmina. A mudança estrutural é uma única ação de Undo/Redo; a centralização não integra o Histórico.
- `Duplicar Lâmina` insere imediatamente depois da origem uma nova Lâmina independente com Background, Overlay, Frames, Fotos ou placeholders, estilos, ajustes, Pilha visual, Último Layout aplicado e estado de Layout travado.
- A duplicação reutiliza os mesmos vínculos externos sem copiar Arquivos originais; alterações posteriores em qualquer das Lâminas não se propagam para a outra.
- O comando fica indisponível quando a origem é uma extremidade de Página única. Em uma duplicação válida, papéis e Numeração são recalculados, a nova Lâmina é centralizada e a operação inteira forma uma única ação de Undo/Redo.
- Excluir uma Lâmina é permitido sem confirmação somente enquanto restarem ao menos duas; com exatamente duas, o comando fica desabilitado.
- A exclusão remove a Lâmina e suas ocorrências, mas preserva os itens importados nas abas `Fotos` e `Decorativos` e nunca apaga seus arquivos externos.
- A operação inteira é uma única ação de Undo/Redo que restaura a Lâmina e sua composição.
- Papéis e Numeração são recalculados; a Lâmina seguinte é centralizada ou, ao excluir a última, a anterior. O Painel de Layouts fecha se apontava para o alvo removido. Esses ajustes de interface não entram no Histórico.
- A Dimensão da Lâmina é física, compartilhada por todo o Álbum e corresponde à superfície total de uma Lâmina dupla e de sua saída `Por lâmina`. A Dimensão da Página é metade da largura da Lâmina, conserva sua altura e governa saídas `Por página` ou de uma Página única.
- O Projeto usa uma única Unidade de medida, entre `mm`, `cm` e `in`, para todas as medidas físicas. Trocar a Unidade converte valores sem alterar tamanhos reais.
- A Resolução do Projeto é expressa em DPI, começa em 300 DPI, governa a renderização das Exportações e pode ser alterada depois da criação sem mudar a composição física.
- A Mudança dimensional segura transforma proporcionalmente Frames e ajustes de Fotos. Alterações fora da faixa visualmente segura são bloqueadas.

### Recorte, segurança e visualização

- Sangria e Área de segurança possuem um valor uniforme cada para todo o Projeto. Ambos começam no equivalente físico a `3 mm` e podem ser definidos como zero independentemente.
- O campo `Sangria` define a largura da Sangria do Projeto; a Área de corte resulta dela e nunca é configurada separadamente.
- A Sangria é medida da borda externa para dentro. A Área de segurança é medida a partir da linha de corte e acumula o recuo da Sangria.
- Os dois recuos ficam dentro da Dimensão da Lâmina e nunca aumentam a largura ou a altura do Projeto ou da Exportação.
- A divisão central de uma Lâmina dupla não recebe Sangria ou segurança adicionais.
- Em uma Página única, a borda voltada ao lado inativo também não recebe Sangria ou segurança.
- Valores que eliminem a Área de corte ou a Área de segurança de alguma Página ativa são inválidos e bloqueados.
- A visualização normal mascara somente a Sangria; o conteúdo permanece no Projeto.
- O Modo de edição da Lâmina mostra toda a superfície ativa, incluindo Sangria e linhas-guia de corte e segurança; o lado inativo permanece oculto e sem interação.
- As linhas são apenas guias: não limitam Frames ou imagens, não recortam a composição e nunca aparecem na Exportação.
- Recorte e segurança podem ser alterados depois da criação. A mudança preserva a composição, atualiza máscara e guias, participa de Undo/Redo e exige Salvamento.

### Composição visual e herança

- A Pilha visual é fixa e exaustiva: Background atrás, Frames com suas Fotos ao centro e Overlay à frente.
- Somente Frames podem ser reordenados entre si dentro da camada intermediária.
- O menu de contexto do Frame e `Editar > Organizar` oferecem `Trazer para frente`, `Avançar uma posição`, `Recuar uma posição` e `Enviar para trás`.
- `Ctrl` + `]` avança uma posição e `Ctrl` + `[` recua uma posição. Levar diretamente ao primeiro ou último nível não possui atalho próprio.
- `Avançar uma posição` e `Recuar uma posição` movem cada bloco contíguo de Frames selecionados através do Frame não selecionado adjacente naquela direção; blocos no limite permanecem sem bloquear os demais.
- `Trazer para frente` e `Enviar para trás` reúnem todos os Frames selecionados em um único bloco no extremo correspondente.
- Toda reordenação coletiva preserva a ordem relativa dos selecionados e dos não selecionados, constitui uma única ação de Undo/Redo e permanece permitida em Layout travado.
- Background é obrigatório e branco por padrão; pode ser uma cor ou uma imagem.
- Overlay é opcional e preserva o canal de transparência de sua imagem.
- Background e Overlay escolhem seus escopos independentemente em cada Lâmina: uma aplicação de Ambos os lados ou aplicações separadas por lado ativo.
- Arrastar um Decorativo sem modificador aplica-o como Background; manter `Shift` pressionado durante o gesto aplica-o como Overlay.
- A posição da soltura determina o escopo: lado esquerdo, lado direito ou Ambos os lados ao usar a região central. `Ambos os lados` cobre os dois lados ativos da mesma Lâmina.
- A região de Ambos os lados é uma faixa proporcional ao redor da junção central, visível durante o arraste e larga o suficiente para não exigir que o usuário acerte a linha divisória. A proporção exata será calibrada no protótipo.
- Durante o arraste, a interface destaca o papel atual e a região de destino; pressionar ou soltar `Shift` troca imediatamente o preview entre Background e Overlay.
- O preview renderiza temporariamente o próprio Decorativo na Pilha visual e no escopo que seriam aplicados: Background abaixo dos Frames ou Overlay acima deles com transparência preservada. Ele substitui visualmente apenas o uso de destino.
- O preview não altera estado, marco de Salvamento ou Histórico. Somente a soltura válida cria uma ação; `Esc` ou soltar fora de um alvo válido cancela e restaura a composição anterior.
- Frames e Fotos não recebem nem interceptam um arraste originado em `Decorativos`. A posição do ponteiro continua sendo resolvida contra a zona da Lâmina subjacente, sem substituir Fotos, preencher Frames ou alterar a seleção atual.
- Uma aplicação manual por arraste torna `custom` somente o papel e o escopo atingidos e é registrada como uma única ação de Undo/Redo.
- Em Lâminas de Página única, o lado desativado não oferece alvo e a faixa central não aparece. Arrastar sobre a Página ativa cria uma aplicação específica daquele lado.
- Dois cliques em um Decorativo ainda criam uma aplicação de Ambos os lados em Página única: ela ocupa somente o lado ativo enquanto a Lâmina permanecer assim e se expande aos dois lados caso seja convertida para dupla.
- Aplicar em um lado altera somente esse lado. Se o papel possuía uma aplicação de Ambos os lados, ela é dividida logicamente sem alterar nem reesticar a aparência do lado oposto; o lado não atingido preserva seu conteúdo e sua origem `default` ou `custom`.
- Aplicar na região central remove as aplicações separadas daquele papel e as substitui por uma única aplicação `custom` de Ambos os lados. Toda a conversão entra no Histórico como uma única ação.
- Imagens de Background e Overlay são esticadas ou comprimidas nos dois eixos até preencher exatamente sua área. Não oferecem pan, zoom ou preservação obrigatória de proporção no escopo inicial.
- O Padrão visual do Projeto define conteúdo e escopo de Background e Overlay.
- Aplicações `default` herdam mudanças posteriores do padrão. Qualquer edição manual transforma somente o alvo editado em `custom`.
- Os nomes `default` e `custom` pertencem ao modelo e à documentação técnica, não ao vocabulário apresentado ao usuário.
- Restaurar o padrão elimina a personalização e reaplica imediatamente o padrão atual.
- Remover localmente um Overlay herdado cria uma ausência personalizada.
- Remover localmente um Background herdado cria um Background branco personalizado.
- Personalizar apenas um lado de uma aplicação herdada de Ambos os lados divide a origem: o lado editado fica personalizado e o outro continua herdado, preservando imediatamente a mesma porção visual que possuía antes da divisão.
- Se o escopo do padrão mudar enquanto houver personalização em apenas um lado, a personalização continua preservada e a parte herdada acompanha o novo padrão.

### Frames e Fotos

- Um Frame é uma máscara retangular alinhada aos eixos, definida por posição, largura e altura, limitada à superfície ativa.
- Um Frame contém no máximo uma Foto e pode atravessar a divisão central somente quando ambos os lados da Lâmina estão ativos.
- A primeira versão permite uma Seleção de Frames com um ou vários elementos no Modo de edição; a seleção é transitória, não participa de Undo/Redo e não é persistida no Projeto.
- Ao entrar no Modo de edição, o Frame selecionado no modo normal é preservado somente se pertencer à Lâmina isolada; caso contrário, a Seleção de Frames começa vazia.
- Sair com `Esc` limpa toda a Seleção de Frames.
- Um clique simples substitui a seleção pelo Frame atingido; `Ctrl` + clique alterna a presença desse Frame na seleção.
- Ultrapassar o limiar padrão de arraste da plataforma sobre um Frame já selecionado preserva a seleção e move o grupo inteiro; sobre um Frame não selecionado, substitui a seleção por ele antes de movê-lo.
- Soltar antes desse limiar executa somente o clique normal e não inicia movimento.
- Undo/Redo remove da seleção qualquer Frame que deixe de existir, preserva os selecionados ainda válidos e nunca seleciona automaticamente um Frame restaurado.
- Em uma sobreposição, clique simples e `Ctrl` + clique atingem somente o Frame mais acima na Pilha visual; não existe gesto adicional para percorrer Frames encobertos.
- Um clique em área vazia limpa a Seleção de Frames. A primeira versão não possui Caixa de seleção de Frames.
- Na seleção múltipla, os contornos individuais permanecem visíveis e uma Caixa delimitadora única envolve o conjunto.
- Arrastar qualquer Frame selecionado move todos os selecionados como grupo e preserva suas distâncias relativas.
- A Caixa delimitadora possui oito alças: laterais escalam somente um eixo; cantos escalam largura e altura independentemente.
- O lado ou canto oposto à alça permanece como âncora, e posições e dimensões de todos os Frames são escaladas proporcionalmente dentro da Caixa.
- O redimensionamento para antes de inverter a Caixa, sair da superfície ativa ou reduzir algum Frame abaixo do mínimo calibrado no protótipo.
- `Shift` em uma alça de canto preserva a proporção da Caixa; `Alt` em qualquer alça redimensiona a partir do centro; `Shift + Alt` combina ambos em uma alça de canto.
- Esses modificadores valem para Frame individual e seleção múltipla e atualizam o gesto enquanto forem pressionados ou soltos.
- Se qualquer Frame ultrapassaria a superfície ativa, movimento e redimensionamento limitam o grupo inteiro no ponto válido mais próximo, sem ajustar elementos individualmente nem alterar suas relações.
- A superfície válida é a Lâmina inteira em Lâmina dupla e somente a Página ativa em Página única.
- Cada movimento ou redimensionamento coletivo completo constitui uma única ação de Undo/Redo.
- O Travamento de Layout abrange todos os Frames da Lâmina; portanto, a seleção no Modo de edição não mistura Frames travados e destravados.
- Em Layout travado, seleção simples e múltipla permanecem disponíveis, com contornos e Caixa delimitadora, mas sem alças de redimensionamento.
- Tentar arrastar uma seleção travada apenas apresenta feedback de bloqueio: não modifica a geometria e não cria ação no Histórico. Substituição de Foto, Borda, Opacidade, Pan, Zoom, Giro, Ângulo, Espelhamento e efeitos permanecem editáveis.
- Para uma seleção múltipla, o Painel contextual mostra a quantidade de Frames, Fotos e placeholders, sem preview de uma Foto individual, e mantém somente controles aplicáveis em lote.
- Uma propriedade numérica igual entre todos os elementos compatíveis mostra o valor compartilhado. Se os valores diferirem, o controle mostra `—` como estado indeterminado, sem sugerir valor zero nem alterar o Projeto.
- Uma propriedade binária divergente mostra estado neutro, sem indicar ligado ou desligado; cores divergentes mostram uma amostra vazia. Esses estados não alteram o Projeto.
- Alterar um controle divergente aplica o novo valor absoluto igualmente a todos os elementos compatíveis e constitui uma única ação de Undo/Redo.
- Borda e Opacidade são propriedades do Frame e, em lote, atingem todos os Frames selecionados, inclusive placeholders.
- Zoom, Ângulo, Giro de 90°, Espelhamento e Preto e branco são propriedades da Foto e atingem somente os Frames selecionados que contêm Foto.
- O Painel informa quando uma propriedade de Foto alcança apenas parte da seleção, com a contagem de Fotos e Frames. Se nenhuma Foto estiver selecionada, os controles exclusivos de Foto ficam ocultos.
- Toda alteração em lote constitui uma única ação de Undo/Redo, mesmo quando placeholders são ignorados por uma propriedade exclusiva de Foto.
- Como regra geral, qualquer comando executado sobre uma Seleção de Frames atua em todos os elementos compatíveis e constitui uma única ação de Undo/Redo; restrições específicas do comando e dos elementos continuam valendo.
- `Trocar conteúdo dos Frames` aparece em `Editar` e no menu de contexto quando exatamente dois Frames estão selecionados e ao menos um contém Foto; dois placeholders desabilitam o comando.
- Com duas Fotos, suas ocorrências completas trocam de Frame. Com uma Foto e um placeholder, a Foto é movida e o Frame de origem torna-se placeholder.
- Posição, dimensões, ordem e estilo permanecem nos respectivos Frames. Cada Foto leva seu vínculo e seus ajustes não destrutivos; o enquadramento é limitado apenas quando necessário para preservar o Preenchimento do novo Frame.
- A troca funciona também em Layout travado e constitui uma única ação de Undo/Redo, pois nunca altera quantidade ou geometria dos Frames.
- No Modo de edição, `Editar > Copiar` e o atalho fixo do MVP `Ctrl + C` copiam toda a Seleção de Frames com geometrias, ordem relativa, Fotos ou placeholders, estilos e ajustes não destrutivos.
- Copiar não altera o Projeto nem cria Histórico. A seleção copiada permanece disponível ao navegar para outra Lâmina do mesmo Projeto, e a Lâmina atualmente isolada no Modo de edição é o destino de `Editar > Colar` ou do atalho fixo do MVP `Ctrl + V`.
- Ao colar na Lâmina de origem, o deslocamento efetivo das novas ocorrências é o menor entre o deslocamento visual desejado e o deslocamento que ainda mantém todo o conjunto na superfície ativa. Se nenhum deslocamento for viável, as cópias permanecem na mesma posição, tornam-se a seleção atual e entram acima dos originais na Pilha visual por uma ordem determinística.
- Quando o destino é uma Página única e a superfície de origem é diferente, posições e dimensões de todo o conjunto são mapeadas proporcionalmente para a Página ativa. Fotos ou placeholders, ordem, estilos e ajustes permanecem associados aos Frames resultantes.
- Quando a origem é uma Página única e o destino é uma Lâmina dupla, o conjunto é mapeado proporcionalmente somente para a Página do mesmo lado lógico: direita permanece direita e esquerda permanece esquerda. A colagem não amplia o conjunto para a Lâmina inteira.
- Nos dois casos, a colagem reutiliza os mesmos vínculos externos, sem duplicar Arquivos ou Cache.
- A colagem preserva as relações internas do conjunto, substitui a seleção anterior pelas novas ocorrências e constitui uma única ação de Undo/Redo.
- A área de transferência de Frames é exclusiva da Janela do Projeto: uma cópia não pode ser colada em outro Projeto aberto.
- Em Layout travado, copiar permanece permitido e colar fica indisponível porque a criação mudaria a quantidade de Frames.
- Uma Foto sempre existe dentro de um Frame. O mesmo Arquivo vinculado pode aparecer em vários Frames com ajustes independentes.
- Um Frame vazio intencional é um Frame placeholder. Qualquer Exportação que inclua sua Lâmina é bloqueada.
- Uma Lâmina sem Frames é válida e exportável.
- A criação manual de um Frame placeholder ocorre somente no Modo de edição da Lâmina por `Editar > Adicionar Frame` ou pelo menu de contexto da área vazia do Canvas.
- O comando cria imediatamente um único Frame centralizado, selecionado e com dimensões proporcionais à superfície ativa, sem modo de desenho nem ferramenta persistente.
- Em Lâmina dupla, o Frame usa a Lâmina inteira como referência e pode atravessar a divisão; em Página única, usa somente a Página ativa.
- A proporção inicial exata é calibrada no protótipo e nunca representa um tamanho físico fixo. A criação é uma única ação de Undo/Redo e fica indisponível em Layout travado.
- Fora do Modo de edição, inserir uma Foto em área livre cria um Frame e aplica o primeiro Layout compatível quando a organização está destravada.
- Inserir uma Foto sobre um placeholder preenche o Frame existente sem mudar sua geometria.
- Arrastar uma Foto sobre qualquer Frame usa somente esse alvo: preenche um placeholder ou substitui a Foto existente, preservando geometria e estilo.
- Se vários Frames contiverem o ponto da soltura, somente o mais acima na Pilha visual é atingido, ainda que esteja vazio, transparente ou com Opacidade reduzida.
- Arrastar para área vazia cria um novo Frame: no Modo de edição, usa a geometria proporcional padrão centrada na soltura e limitada por deslocamento à superfície ativa; no modo normal, o primeiro Layout compatível decide a geometria.
- Layout travado aceita arraste de Foto somente sobre Frames existentes.
- O duplo clique no Painel preenche o placeholder mais à esquerda da Lâmina de destino. Somente quando não houver placeholder cria um novo Frame; no Modo de edição, ele usa a mesma geometria centralizada e proporcional da Criação manual de Frame.
- Entre placeholders, menor borda esquerda prevalece e, em empate, menor borda superior.
- Em Layout travado, o duplo clique só é válido quando houver placeholder; caso contrário, não altera o Projeto.
- O Frame preenchido, substituído ou criado torna-se a única seleção; isso não adiciona outra entrada ao Histórico.
- No Modo de edição, inserções e exclusões não aplicam Layout automaticamente.
- Em Layout travado, uma nova ocorrência de Foto só pode preencher um placeholder existente. Arrastar explicitamente sobre um Frame preenchido pode substituir sua Foto porque não altera a quantidade nem a geometria dos Frames.
- Excluir um Frame em organização destravada remove conjuntamente o Frame e sua Foto. Fora do Modo de edição, a quantidade restante recebe o primeiro Layout compatível; dentro dele, as geometrias restantes são preservadas.
- No Modo de edição, `Delete` ou `Excluir` no menu de contexto remove toda a Seleção de Frames sem confirmação e como uma única ação de Undo/Redo.
- Remover uma Foto de um Layout travado preserva o Frame como placeholder. Excluir a estrutura exige destravar o Layout.
- Quando `Delete` atua sobre uma seleção em Layout travado, remove somente as Fotos, preserva todos os Frames como placeholders e deixa placeholders já vazios inalterados, em uma única ação.
- Substituir uma Foto preserva geometria e estilo do Frame, mas reinicia a nova ocorrência centralizada, colorida, sem espelhamento, com Giro de 90° em `0°`, Ângulo em `0°` e Zoom do usuário em `1×`.
- A Borda do Frame é opcional, possui cor e espessura na Unidade do Projeto, é desenhada para dentro e reduz somente a área visível da Foto.
- O Padrão de Frame define presença, cor e espessura da Borda. Frames novos e herdados acompanham mudanças nesse padrão.
- A Opacidade do Frame varia entre `0%` e `100%`, afeta conjuntamente Foto e Borda, começa em `100%` e não pertence ao Padrão de Frame.
- Qualquer alteração manual de Borda ou Opacidade torna o estilo completo do Frame `custom`.
- Restaurar o padrão do Frame reaplica a Borda atual, redefine Opacidade para `100%` e retoma a herança.
- Fotos nunca podem revelar áreas vazias dentro do Frame. O Zoom de preenchimento é recalculado sempre que necessário.
- O Zoom do usuário é um multiplicador adicional iniciado em `1×` e nunca reduz a Foto abaixo do Zoom de preenchimento.
- O gesto direto de enquadramento existe no modo normal: `Alt` + arraste faz Pan e `Alt` + roda altera o Zoom da Foto no Frame sob o ponteiro. No Modo de edição, os gestos diretos manipulam a geometria do Frame.
- Durante o Pan, a porção da Foto fora do Frame é exibida temporariamente com opacidade reduzida, sem diminuir a opacidade da porção interna, e o Frame apresenta as quatro linhas-guia da regra dos terços. Os dois auxílios desaparecem ao encerrar o gesto e nunca participam do Projeto, Undo/Redo ou Exportação.
- Gestos diretos de Pan/Zoom não alteram a seleção. Cada arraste consolidado e cada sequência contínua da roda formam, respectivamente, uma única ação de Undo/Redo.
- Os ajustes iniciais exaustivos da Foto são Pan, Zoom do usuário, espelhamento horizontal, Giro anti-horário em passos de 90 graus, Ângulo contínuo entre `-45°` e `+45°` e preto e branco.
- Giro de 90° e Ângulo são valores independentes. A ordem normativa é Giro de 90°, Ângulo, Espelhamento horizontal, Zoom de preenchimento, Zoom do usuário, Pan e Efeitos. Alterar Giro, Ângulo ou Espelhamento recalcula o preenchimento e os limites de Pan.
- Redimensionar um Frame recalcula o preenchimento e preserva proporcionalmente Zoom do usuário, Pan e ponto focal quando possível, limitando-os apenas para impedir vazamentos.
- Todos os ajustes são não destrutivos e nunca modificam o Arquivo vinculado original.

### Layouts

- O Painel de Layouts é aberto exclusivamente pelo controle central da Barra de uma Lâmina no modo normal e permanece associado a esse alvo explícito até ser fechado ou redirecionado por outra Barra. Não pode ser aberto nem utilizado no Modo de edição.
- A entrada no Modo de edição oculta temporariamente uma faixa aberta sem perder seu alvo. A saída restaura a faixa e recalcula seus Layouts compatíveis antes de exibi-la.
- O Painel de Layouts possui duas seções horizontais: `Automáticos` contém Layouts produzidos pelo Gerador de Layouts e `Personalizados` contém Layouts criados pelo usuário e disponíveis no catálogo global.
- O hover sobre uma preview executa um Mapeamento transitório e renderiza os próprios Frames da Lâmina alvo nas posições/dimensões candidatas, preservando Fotos, placeholders, estilos, ordem e ajustes.
- Em candidatos de travamento com posições excedentes, o hover mostra essas posições como placeholders vazios transitórios, com o Padrão de Frame herdado, sem criá-los no Projeto; o corpo da preview não os aplica, e somente o cadeado pode confirmar a operação.
- O enquadramento das Fotos é recalculado com o mesmo caminho da aplicação real. A prévia não modifica Projeto, estado de Salvamento ou Undo/Redo.
- Sair da preview restaura a geometria anterior; passar para outra substitui a representação. Clicar confirma exatamente o resultado mostrado, e clicar no cadeado confirma, cria os placeholders excedentes e trava, cada um como uma única ação.
- Um Layout armazena somente uma lista ordenada de posições e dimensões de Frames. Não armazena Fotos, estilos, Background ou Overlay.
- Aplicar um Layout copia sua geometria para a Lâmina. A Organização aplicada não mantém referência viva ao item de catálogo.
- Um Layout destravado realiza uma organização única; depois disso, o usuário pode mover e redimensionar Frames sem influência do Layout.
- Travar um Layout congela quantidade, posição e dimensões de todos os Frames da Lâmina: impede criar ou excluir estruturas de Frame, movê-las ou redimensioná-las. Seleção, substituição de Foto, Borda, Opacidade, Pan, Zoom, Giro, Ângulo, Espelhamento, efeitos e ordem visual continuam editáveis.
- O cadeado na prévia do Layout aplica e trava a organização no mesmo fluxo; não existe uma tela separada para travamento.
- Em uma Lâmina travada, a preview aplicada fica destacada com o cadeado fechado e todas as outras previews ficam desabilitadas.
- Na navegação comum, somente Layouts com a mesma quantidade total de Frames são oferecidos.
- No próprio Painel de Layouts, candidatos de travamento também podem ter mais posições que a quantidade atual; Layouts com menos posições não são apresentados como candidatos válidos.
- Ao travar, um Layout com a mesma quantidade reorganiza e trava; posições excedentes criam placeholders; um Layout com menos posições é incompatível.
- O Mapeamento de Layout associa Frames pela ordem atual na Pilha visual às posições ordenadas, preservando Foto ou ausência e estilo.
- Clicar no cadeado fechado da preview destacada destrava imediatamente, sem confirmação, reabilita as outras previews e preserva Frames, Fotos, placeholders, estilos, ajustes, ordem e geometria.
- Enquanto a organização estiver travada, nenhuma ação pode aplicar outra geometria de Layout; o usuário deve destravá-la antes de trocar de organização.
- Travar e destravar constituem ações de Undo/Redo.
- O último Layout aplicado é uma cópia local de sua geometria original e de sua categoria de origem. Permanece primeiro dentro dessa seção enquanto compatível e pode ser reaplicado depois de edições manuais ou da remoção da origem global.
- Dentro de cada seção, a ordem é: Último Layout aplicado, quando pertencer à categoria; Favoritos do Projeto; e demais candidatos. Uma definição possui somente uma preview, mesmo quando é simultaneamente a última aplicada e favorita.
- Para aplicação automática, a prioridade global é: Último Layout aplicado compatível, primeiro Favorito do Projeto, primeiro Layout personalizado global e primeiro Layout do sistema. Dentro de cada grupo, prevalece a ordem exibida em sua seção.
- Layouts do sistema são produzidos pelo Gerador de Layouts.
- A existência de ao menos uma organização compatível é garantida para qualquer quantidade suportada de Frames, formato de superfície e escopo. Quando nenhum candidato do catálogo, dos Favoritos ou do Gerador servir, o aplicativo usa um arranjo de reserva derivado apenas da quantidade de Frames e da superfície ativa. Esse arranjo não aparece como preview no Painel de Layouts e não pode ser favoritado.
- Layouts personalizados são salvos imediatamente no catálogo global a partir da geometria e da ordem atuais dos Frames, no Modo de edição da Lâmina, pelo botão em `Design da Lâmina` ou pelo comando equivalente no menu `Editar`.
- A criação não pede nome nem abre modal; a preview geométrica identifica o item, o escopo é inferido automaticamente e a ação não sai do Modo de edição, aplica outra geometria ou trava a composição.
- Criar ou excluir um Layout personalizado persiste imediatamente no catálogo do aplicativo, não marca o Projeto como alterado e não participa de seu Undo/Redo.
- Criação e exclusão substituem atomicamente a revisão do catálogo global. Cada Janela consulta a revisão vigente ao abrir o Painel de Layouts, receber foco ou solicitar atualização manual; broadcast imediato entre Janelas não é requisito do MVP.
- Atualizar o catálogo não altera composição, estado de Salvamento ou Undo/Redo de qualquer Projeto receptor.
- A criação exige ao menos um Frame. Sem Frames, botão e item de menu ficam desabilitados com uma explicação curta.
- Dois Layouts personalizados são duplicados quando possuem o mesmo escopo, tipo e proporção de superfície, quantidade de Frames e a mesma sequência ordenada de posições e dimensões normalizadas. Uma ordem visual diferente representa outra identidade de Layout.
- Tentar salvar uma duplicata não modifica o catálogo, mostra um aviso não modal e faz a preview existente ser localizada e brevemente realçada na próxima abertura compatível do Painel de Layouts no modo normal.
- Previews automáticas oferecem estrela e cadeado; previews personalizadas oferecem também uma lixeira.
- A lixeira exige confirmação e remove somente o Layout personalizado do catálogo global. Organizações aplicadas, Últimos Layouts aplicados e Favoritos do Projeto permanecem intactos; o Projeto aberto não é modificado.
- A estrela pode favoritar um Layout automático ou personalizado. Favoritar cria uma cópia completa e estável dentro do Projeto, e a estrela preenchida indica sua existência.
- Favoritar não move a preview entre `Automáticos` e `Personalizados`; a categoria continua representando a origem do Layout. Alterar ou excluir a origem global não afeta o favorito.
- Clicar na estrela alterna favoritar/desfavoritar imediatamente, sem confirmação. A ação marca o Projeto como alterado, participa de Undo/Redo e só é persistida pelo Salvamento manual.
- Desfavoritar remove apenas a cópia do Projeto e não altera Organizações aplicadas. Sem origem disponível, a preview só permanece no painel de uma Lâmina que ainda a conserva como Último Layout aplicado.
- Layouts copiados para um Projeto acompanham Cópias de Projeto e Geração de Projetos em lote.
- Layout por Lâmina pode permitir Travessia central.
- Layout por Página produz uma organização global formada por Blocos de Frames centralizados nas Páginas e nunca permite que um Frame atravesse o centro.
- Ao salvar um Layout personalizado, a existência de qualquer Travessia central determina escopo por Lâmina; se todos os Frames estiverem integralmente em um dos lados, o escopo é por Página.
- Compatibilidade exige o mesmo tipo de superfície e a mesma proporção. Diferenças de tamanho físico, Unidade ou DPI são acomodadas por escala proporcional.
- Converter uma Lâmina de extremidade entre dupla e página única preserva Fotos, placeholders e estilos, descarta a geometria anterior e destrava a organização. Se houver Frames, aplica o primeiro Layout compatível; se não houver, mantém a Lâmina sem Layout.
- Na conversão de Lâmina dupla para Página única, aplicações de Background e Overlay por lado preservam o lado que continua ativo e descartam o lado inativo; aplicações de Ambos os lados são reajustadas à área ativa.
- Na conversão de Página única para Lâmina dupla, aplicações por lado preservam o lado existente e iniciam o novo lado em `default`; aplicações de Ambos os lados são reajustadas aos dois lados.

### Painel de imagens e arquivos externos

- O Painel de imagens possui abas independentes `Fotos` e `Decorativos`; a aba ativa determina a categoria de cada importação.
- `Importar` oferece `Arquivos...`, com seleção múltipla no diálogo do Windows, e `Pasta...`, com seleção de uma pasta.
- A importação comum por pasta considera somente arquivos diretamente contidos nela e nunca percorre subpastas; a descoberta recursiva permanece exclusiva dos fluxos em lote que a especificam.
- O Painel de imagens também aceita arquivos e pastas arrastados do sistema operacional. A soltura usa a aba ativa e, para cada pasta, considera somente os arquivos diretamente contidos nela.
- JPG/JPEG, PNG e TIFF/TIF são os formatos importáveis iniciais. Outros formatos não são importados.
- Uma importação múltipla possui sucesso parcial: arquivos válidos são importados, enquanto arquivos inválidos, corrompidos ou incompatíveis são rejeitados sem reverter os sucessos.
- Duplicatas seguem a regra normal de reimportação e não são classificadas como falha.
- Quando houver rejeições, a Tela de Problemas é aberta ao final com `Arquivo` e `Motivo`; fechá-la não remove os itens importados.
- Cada seleção de arquivos, escolha de pasta ou única soltura agrupa todos os novos vínculos aceitos em uma única ação de Undo/Redo e deixa o Projeto com alterações pendentes.
- Undo remove somente os itens criados naquela operação; não modifica os arquivos originais nem duplicatas preexistentes. Redo restaura os vínculos importados.
- Uma operação que não acrescente nenhum item novo não cria Histórico nem marca o Projeto como alterado.
- O mesmo caminho pode existir uma vez em cada aba como itens distintos, mas não pode ser duplicado dentro da mesma aba.
- Reimportar um caminho já presente na aba apenas seleciona o item existente.
- A busca do Painel filtra em tempo real pelo Nome do arquivo na aba ativa e ignora diferenças entre maiúsculas, minúsculas e acentos.
- A busca é combinada por interseção com o Filtro de uso e qualquer outro filtro ativo, sem alterar a Ordenação escolhida.
- Cada aba mantém seu próprio texto somente enquanto a Janela do Projeto está aberta. O controle `X` limpa o texto da aba atual.
- O texto da busca não integra o Projeto, o Histórico ou as preferências restauradas na próxima sessão.
- Um slider único ajusta continuamente o tamanho das miniaturas da aba ativa e reorganiza a grade em tempo real.
- As miniaturas preservam a proporção inteira da imagem, sem corte. Dois cliques no slider restauram o tamanho médio padrão.
- `Fotos` e `Decorativos` mantêm tamanhos independentes, persistidos como preferências globais do usuário e reutilizados entre Projetos e sessões sem alterar o Projeto ou o Histórico.
- Os valores exatos de mínimo, máximo e tamanho médio serão calibrados no protótipo.
- Um clique simples substitui a seleção do Painel e estabelece sua âncora; `Ctrl` + clique adiciona ou remove uma mídia individualmente.
- `Shift` + clique seleciona o intervalo contínuo entre a âncora e o item acionado conforme a ordem atualmente visível depois da Busca, dos filtros e da Ordenação.
- `Ctrl + A` seleciona exclusivamente todos os itens atualmente visíveis na aba ativa; itens ocultos pela Busca ou pelos filtros não são incluídos.
- Quando uma mudança de Busca ou filtro oculta uma mídia já selecionada, ela e sua eventual âncora são retiradas imediatamente da seleção. Mídias que permanecem visíveis continuam selecionadas.
- Alterar apenas a Ordenação preserva os mesmos itens selecionados e a mesma âncora em suas novas posições visuais.
- Clicar com o botão direito sobre uma mídia já selecionada preserva todo o grupo; clicar sobre uma mídia fora da seleção substitui o grupo somente por ela antes de abrir o menu.
- Com foco no Painel, `Delete` e `Remover` no menu de contexto executam a remoção sobre a seleção resultante. Fora dele, `Delete` continua obedecendo ao contexto ativo do Canvas.
- O Painel não oferece Caixa de seleção. Arrastar ou dar dois cliques atua somente sobre a mídia diretamente acionada, nunca sobre toda a seleção múltipla.
- A seleção do Painel é transitória, não integra o Projeto e não participa de Undo/Redo.
- Um item Decorativo pode ser usado como Background ou Overlay; o papel pertence ao uso, não ao arquivo.
- No arraste de um Decorativo, o uso padrão é Background e `Shift` transforma o mesmo gesto em aplicação de Overlay.
- As zonas esquerda, central e direita da Lâmina determinam respectivamente os escopos lado esquerdo, Ambos os lados e lado direito.
- Durante esse arraste, Frames e Fotos são transparentes ao roteamento do gesto: o alvo é sempre a zona da Lâmina sob o ponteiro.
- Dois cliques em um Decorativo aplicam-no a Ambos os lados da Lâmina usada como alvo implícito: Background sem modificador e Overlay com `Shift`.
- A Ordenação do Painel oferece Nome, Data de criação e Data de alteração, em direção crescente ou decrescente.
- A ordenação padrão é Nome crescente com comparação natural. Datas vêm do Arquivo vinculado; Arquivos ausentes ficam no fim e empates usam Nome natural.
- O Filtro de uso oferece `Todas`, `Usadas` e `Não usadas`.
- Uma Foto está usada quando aparece em algum Frame. Um Decorativo está usado quando está aplicado ou configurado no Padrão visual do Projeto.
- Ordenação e filtro são preferências do aplicativo por aba, reutilizadas entre Projetos e sessões. Não alteram o Projeto e não participam de Undo/Redo.
- Remover uma seleção de Fotos sem uso retira diretamente os itens do Painel. Se ao menos uma Foto estiver em uso, um único diálogo consolidado para toda a seleção oferece `Remover tudo`, `Remover imagens e manter os Frames` ou `Cancelar`.
- Em `Remover tudo`, Frames destravados são removidos com suas Fotos; Frames pertencentes a Layouts travados permanecem como placeholders sem destravar.
- Em `Remover imagens e manter os Frames`, todas as ocorrências usadas das Fotos selecionadas são esvaziadas e todos os Frames afetados permanecem como placeholders, estejam travados ou destravados; itens selecionados sem uso apenas saem do Painel.
- Um único Decorativo sem uso pode ser removido diretamente. Um Decorativo em uso ou qualquer seleção múltipla de Decorativos exige uma única confirmação conjunta; nunca é exibido um diálogo por item.
- Toda remoção confirmada, individual ou em lote, constitui uma única ação de Undo/Redo e nunca remove o Arquivo original.
- Usos locais de Decorativos removidos voltam para `default`.
- Se o Decorativo removido for o padrão, Background padrão passa a branco ou Overlay padrão passa a ausente; aplicações herdadas acompanham e personalizações permanecem intactas.
- Arquivos permanecem externos e são acessados por seus caminhos originais; o Projeto não incorpora nem copia mídia.
- Localizações de Projeto, Arquivos vinculados, origens e Destinos podem usar caminhos locais absolutos, UNC, unidades mapeadas, verbatim locais ou verbatim UNC. Entradas externas relativas ao processo, namespaces de dispositivos, curingas, fluxos alternativos de dados e componentes reservados são rejeitados; alvos existentes têm tipo confirmado após a abertura, e alvos novos exigem pai validado e confirmação pelo handle depois da criação.
- Caminhos são tratados em sua representação nativa. A forma textual exibida ou persistida não é usada como identidade física e não é reescrita apenas por uma simplificação interna.
- Cada tentativa com vários arquivos mantém no componente proprietário um contexto transitório e reutiliza nele somente bindings de raiz durante o planejamento. Antes de atravessar processos, o proprietário congela os bindings em um plano imutável; todos os participantes recebem o mesmo plano e nenhum worker resolve novamente por conta própria uma raiz já capturada.
- O plano fixa somente o binding operacional usado pelo aplicativo, não a identidade do servidor físico por trás de DFS, DNS, SMB ou armazenamento equivalente. Plano e contextos são descartados em sucesso, falha ou cancelamento; repetição manual, retomada após reinício ou nova operação resolvem novamente as raízes.
- Reutilizar um binding não substitui a abertura e a validação de cada original ou Destino necessário. Exportação, Photoshop e validações finais continuam acessando os objetos reais.
- Se o conteúdo no mesmo caminho mudar, Projetos vinculados passam a observar a versão atual e invalidam representações antigas de interação.
- Um watcher monitora os Arquivos vinculados dos Projetos abertos e sinaliza possíveis alterações; ele não confirma sozinho conteúdo, ausência ou disponibilidade.
- Eventos rápidos do mesmo caminho são consolidados; uma inspeção autoritativa só começa quando o arquivo estiver estável e legível. Depois da confirmação, o estado observado é atualizado e somente as representações de Cache afetadas são invalidadas.
- Se a origem estiver acessível e a remoção ou renomeação persistir depois da estabilização, o item assume o estado de Arquivo ausente. Se a rede, o servidor, o compartilhamento ou a permissão impedir a confirmação, ele assume Arquivo indisponível sem alterar a referência. Quando o acesso ou o arquivo retornam ao caminho registrado, o estado e as prévias são restaurados automaticamente sem Religação.
- A atualização externa não altera a referência persistida, não entra em Undo/Redo e não marca o Projeto como alterado.
- O Cache de cada Projeto fica em `%LOCALAPPDATA%\MyAlbuns\Cache\{project-id}`, usando a Identidade em vez de Nome ou caminho.
- O baseline do spike para a pasta é `metadata.json` e `Media`, com uma representação reduzida por mídia e sem tiles ou previews de Lâmina persistidas em disco. Se as cenas e o Zoom representativos demonstrarem insuficiência, o relatório do spike revisa esse contrato antes da implementação ampla.
- `metadata.json` é descartável e registra versão do schema, Identidade do Projeto, último uso e, por mídia, dimensões, formato, orientação EXIF, tamanho, datas, quantidade de páginas quando aplicável, perfil de cor básico e fingerprint.
- Cada representação registra também uma versão de suas regras e uma geração única. Versão incompatível invalida a entrada.
- Um único componente é o proprietário lógico dos jobs, índice, gerações, invalidação, pausa e manutenção do Cache. Fora de manutenção, um único Processador de Imagens atua como adaptador escritor de cada namespace. Cada job usa `.tmp` próprio, revalida que pedido, fingerprint e variante continuam atuais e descarta resultados obsoletos.
- O artefato imutável é publicado antes de `metadata.json`; o índice serializado referencia a mesma geração e é substituído por último. Temporários e gerações não referenciadas ou antigas sem consumidores são descartados depois de reinício ou novo acesso.
- Ao abrir ou acessar uma mídia, diferenças de tamanho ou data de alteração invalidam somente sua entrada. Evento do watcher, retorno de Arquivo ausente ou indisponível, Religação, schema incompatível ou artefato inválido também provocam regeneração localizada.
- A abertura comum não recalcula o hash completo de todos os originais. É aceito o risco raro de uma alteração realizada enquanto o aplicativo está fechado conservar exatamente tamanho e data e não ser percebida.
- Pan, Zoom, Frames, Layouts e demais decisões de composição não invalidam a representação reduzida; as mesmas regras determinísticas calculam o plano que a prévia e a saída final adaptam aos seus respectivos renderizadores.
- Um Arquivo ausente ou indisponível pode conservar sua última representação e metadados conhecidos com indicação própria, mas continua inválido como fonte de Exportação.
- Se uma origem acessível confirmar que um arquivo foi movido ou removido, cada Projeto o considera ausente e pode religá-lo independentemente. Arquivo indisponível preserva o vínculo e oferece nova tentativa, não Religação. Uma Religação aceita altera somente a referência daquele Projeto, participa de Undo/Redo, exige Salvamento e nunca move o arquivo original.
- Arquivo ausente usado na seleção bloqueia a Exportação. Arquivo ausente apenas no Painel gera aviso, mas não bloqueia.
- O Cache de mídia serve exclusivamente à interação. Nunca substitui um original ausente e nunca é usado como fonte de Exportação.
- `Liberar espaço` reserva atomicamente namespaces sem proprietário ativo e remove somente Cache de Projetos fechados. `Limpar todo o Cache` executa apenas sem Projeto ou Processador ativo; caso contrário, é agendado para a próxima inicialização segura, sem pausar editores nem remover Cache ativo ao vivo.

### Identidade, sessão e persistência

- Cada Projeto possui Identidade imutável, independente de Nome e Localização.
- A Localização pode ser local, UNC, unidade mapeada ou caminho verbatim local/UNC. Duas representações diferentes podem alcançar o mesmo arquivo físico e não criam, por isso, dois Projetos.
- O Nome do Projeto deriva do nome do arquivo e serve de base para as saídas de Exportação.
- Mover ou renomear o arquivo preserva a Identidade. `Salvar como` cria novo Nome, nova Localização e nova Identidade.
- Mover ou renomear reutiliza os mesmos namespaces de Cache e Recuperação. `Salvar como` começa com Identidade própria, não copia artefatos descartáveis e encerra o checkpoint anterior depois do sucesso.
- Uma Cópia de Projeto contém toda a estrutura, configurações, favoritos e referências, mas não mantém derivação, herança ou sincronização com a origem.
- Uma cópia criada pelo sistema operacional recebe automaticamente nova Identidade quando o original ainda existe e o arquivo copiado pode ser atualizado.
- A identidade de uma Cópia externa é corrigida antes de qualquer acesso a estado local por Projeto, garantindo que original e cópia nunca compartilhem Cache ou Recuperação. Se o arquivo não for gravável, a abertura falha de forma fechada, não monta namespaces sob a Identidade duplicada e oferece orientação para `Salvar cópia como...`.
- Se uma origem acessível confirmar que o caminho anterior não existe, a mudança é tratada como movimentação e preserva a Identidade. Indisponibilidade ou acesso negado não autoriza inferir movimentação nem Cópia externa.
- A correção automática de Identidade é a única escrita técnica permitida no arquivo persistido do Projeto sem Salvar e nunca inclui alterações criativas pendentes. Cache, preferências e Recuperação de sessão usam estados separados.
- O Bloqueio de abertura combina Identidade persistida, comparação física disponível e bloqueio real do arquivo. Tentar abrir novamente o mesmo Projeto pelo mesmo caminho ou por um alias equivalente reutiliza e focaliza a sessão existente; resultado inconclusivo nunca autoriza silenciosamente uma segunda sessão editável.
- Projetos com Identidades diferentes podem permanecer abertos simultaneamente, mesmo quando compartilham referências aos mesmos arquivos.
- Bloqueios órfãos são recuperados automaticamente após encerramento inesperado.
- Alterações criativas só são persistidas por ação explícita do usuário.
- Fechar uma sessão com mudanças pendentes oferece salvar e fechar, descartar e fechar, ou cancelar.
- `Salvar como` grava o estado visível em um novo Projeto e faz a sessão passar a representar esse novo arquivo; o original permanece em sua última versão salva.
- Recuperação de sessão usa estado temporário separado. Ao restaurar, o conteúdo continua não salvo até uma ação explícita.
- A Recuperação de Projeto fica em `%LOCALAPPDATA%\MyAlbuns\Recovery\Projects\{project-id}.json`; a identidade de uma Cópia externa é resolvida antes de consultá-la.
- Depois de `Salvar como` bem-sucedido, o checkpoint da Identidade anterior é removido; mudanças posteriores usam a nova Identidade. Cancelamento ou falha preserva a sessão e o checkpoint anteriores.
- Cada ação criativa concluída agenda a atualização atômica da Recuperação. Ações muito próximas podem ser consolidadas em uma única escrita, e nenhum estado transitório de arraste ou redimensionamento é persistido.
- Uma queda durante um gesto restaura o último estado concluído antes dele. `Salvar` sem novas mudanças e o fechamento normal confirmado removem a Recuperação temporária.
- O checkpoint de Recuperação inclui somente o estado criativo consolidado e o marco da última versão salva; nunca inclui as pilhas de Undo e Redo.
- Recuperar cria uma nova sessão ainda não salva com Undo/Redo vazios. O Histórico normal continua essencial durante uma sessão viva, mas não atravessa uma queda.
- Cada Projeto mantém Undo/Redo dentro de um orçamento automático de memória. Comandos contêm somente deltas de domínio e referências, nunca IDs genéricos de comandos da interface, pixels, Cache ou cópias dos originais.
- Ao alcançar o orçamento, as ações de Undo mais antigas são descartadas primeiro. Estado atual, Redo ainda válido, marco do último Salvamento e indicação de mudanças pendentes permanecem independentes.
- A Recuperação não guarda o Histórico disponível. O orçamento do Histórico em memória será definido pelo spike e não terá configuração manual na primeira versão.
- Uma falha em uma Sessão do Projeto não pode corromper o estado ou a Recuperação de outro Projeto. A sobrevivência das demais Janelas depende da topologia escolhida pelo spike.
- Após a queda, `Reabrir e recuperar` cria uma nova sessão ainda não salva; `Abrir última versão salva` exige confirmação antes de descartar a recuperação; `Agora não` mantém o estado temporário para a próxima abertura.
- Se a topologia permitir que Janelas sobrevivam à indisponibilidade do componente global, edição e Salvamento locais continuam, enquanto operações globais ficam indisponíveis até uma reinicialização explícita protegida pelo singleton; não há eleição ou reinício automático no MVP.
- Undo e Redo abrangem todas as alterações editáveis durante a sessão, continuam disponíveis depois de Salvar e não persistem após fechar.
- Alterar um Projeto nunca modifica o estado pertencente a outro Projeto.

### Exportação

- Exportação é a única ação final do escopo inicial; não existem impressão ou geração de prova.
- JPEG, PNG e PDF são os formatos de saída.
- A tela da Exportação normal organiza `Escopo`, `Modo`, `Formato` e `Destino` em um único diálogo modal e mostra no rodapé um resumo da quantidade de arquivos ou páginas resultantes.
- A tela não permite alterar dimensões ou DPI; sempre usa as configurações do Projeto.
- Quando JPEG está selecionado, a Exportação normal mostra um slider de qualidade. O controle fica ausente para PNG e PDF e não altera Projeto, Salvamento ou Undo/Redo.
- O slider inicia em qualidade máxima a cada abertura do diálogo; dois cliques restauram esse valor, que não é persistido como preferência para a próxima Exportação.
- A Exportação normal pode abranger o Álbum inteiro ou um Intervalo contínuo de Lâminas.
- A ação contextual `Exportar Lâmina` abre a tela de Exportação com início e fim preenchidos pela Lâmina selecionada.
- A Exportação normal usa o estado atual visível, inclusive alterações não salvas, sem salvar o Projeto.
- Somente uma Exportação normal pode permanecer ativa entre todos os Projetos abertos. Não existe fila de espera: enquanto ela executa, todas as ações que iniciariam outra Exportação normal ficam indisponíveis.
- O Projeto exportado permanece bloqueado por seu próprio modal de progresso. O modal pertence somente àquela janela e não aparece, bloqueia ou cobre as janelas dos demais Projetos.
- Outros Projetos continuam disponíveis para edição, Undo/Redo, Salvamento e navegação durante a Exportação; somente suas ações de Exportação normal ficam indisponíveis.
- O Bloqueio global de Exportação normal é liberado ao concluir, falhar ou cancelar a operação ativa.
- A exclusividade usa uma concessão global limitada à tentativa; progresso e cancelamento pertencem somente àquela tentativa e não transformam o mecanismo de exclusividade em coordenador de comandos ou estado criativo.
- Antes de reservar o Processador para a saída final, a Exportação pausa todo trabalho de Cache que compartilharia esse Processador, usando o menor escopo seguro permitido pela topologia. A pausa cobre ao menos o Cache do Projeto exportado e é liberada ao concluir, falhar ou cancelar.
- Toda saída é renderizada a partir dos Arquivos vinculados originais, nunca do Cache.
- Exportação normal e cada item do lote percorrem o mesmo contrato de planejamento, execução e Publicação. A operação recebe um snapshot imutável, não salva nem religa o Projeto e conserva a Publicação limitada documentada.
- A validação considera somente a seleção. Arquivos ausentes ou indisponíveis efetivamente necessários à renderização e placeholders dentro dela bloqueiam; problemas fora dela não bloqueiam. O Filtro de uso do Painel não substitui essa análise de dependências.
- Problemas de diferentes partes do aplicativo usam uma Tela de Problemas tabular reutilizável. Exportação e lotes usam `Projeto`, `Problema` e `Ações`; outros contextos adaptam a entidade e podem omitir ações quando não houver correção disponível.
- A Tela de Problemas é aberta automaticamente pelo fluxo que detectou os itens, já filtrada para seu contexto, e não possui comando permanente em `Exibir` na primeira versão.
- A tela permite acessar os Projetos necessários à correção; a operação de origem permanece pendente e não inicia processamento final enquanto os problemas são tratados.
- Quando a validação da Exportação encontra bloqueios, abre essa tela filtrada para o contexto da operação, antes de criar progresso ou saída.
- Linhas de Frame placeholder oferecem `Abrir Projeto`; linhas de Arquivo ausente oferecem `Relinkar`; linhas de Arquivo indisponível oferecem `Tentar novamente` sem alterar a referência.
- Na Exportação normal, `Relinkar` solicita a pasta das Fotos daquele Projeto e busca recursivamente o nome e a extensão exatos de cada Arquivo ausente. Somente uma correspondência única é aceita; nenhuma ou várias mantêm a pendência. Referências aceitas atualizam a sessão aberta, participam de Undo/Redo e ficam pendentes até o Salvamento manual, mas já podem ser usadas pela Exportação do estado visível.
- A validação é repetida após cada correção. `Continuar Exportação` só é habilitado sem pendências sem decisão e ainda exige um clique explícito; resolver ou ignorar a última linha nunca inicia o processamento automaticamente.
- Fechar a tela cancela a Exportação pendente. Relinks individuais permanecem na sessão como mudanças não salvas, enquanto resoluções temporárias do lote são descartadas.
- A saída inclui toda a superfície da unidade correspondente, inclusive sua Sangria, mas nunca inclui linhas-guia: usa a Dimensão da Lâmina para uma Lâmina dupla e a Dimensão da Página para cada saída `Por página` ou para uma Página única.
- No modo `Por lâmina`, cada Lâmina gera uma unidade conjunta. Uma Página única gera somente a área ativa, sem metade vazia.
- No modo `Por página`, cada lado ativo gera uma unidade. Frames com Travessia central são recortados exatamente na divisão.
- Lados inativos não geram arquivo, espaço em branco, número ou lacuna.
- A Numeração de Página é sequencial somente entre Páginas ativas.
- JPEG e PNG usam o namespace compartilhado `{nome-do-projeto}_{índice com três dígitos}`. O índice é a posição da Lâmina no modo `Por lâmina` e a Numeração de Página no modo `Por página`; o nome isolado não identifica o modo de origem.
- Em uma Exportação parcial, índices preservam suas posições originais e não reiniciam em `001`.
- PDF gera `{nome-do-projeto}.pdf`, com uma página por unidade do modo selecionado.
- O Destino padrão é uma pasta com o Nome do Projeto ao lado do arquivo do Projeto; o usuário pode escolher outro local.
- Destinos locais, UNC, em unidade mapeada ou verbatim local/UNC seguem o mesmo fluxo. Resolução ou acesso de rede nunca executa na thread da interface.
- Todos os conflitos são detectados antes do início e apresentados em um único diálogo com `Sobrescrever todos` ou `Cancelar`.
- Nenhuma saída é renomeada ou sobrescrita silenciosamente.
- Cada tentativa cria uma pasta de preparação reservada dentro da própria pasta de Destino, renderiza e verifica ali todas as saídas selecionadas e só então inicia a publicação.
- Falha ou cancelamento durante a preparação remove seus temporários, não repete automaticamente a Exportação e não modifica os nomes finais. O modal oferece `Tentar novamente` ou `Fechar` e libera o Bloqueio global ao encerrar a tentativa.
- Depois da preparação integral, cada arquivo é promovido ao nome final com atomicidade por arquivo quando o destino suportar. Não existe rollback do conjunto: uma falha durante a publicação pode deixar uma mistura de saídas antigas e novas, deve ser informada explicitamente e nunca autoriza a remoção de órfãos.
- A operação só informa sucesso depois de publicar todo o conjunto. Temporários são removidos no sucesso normal ou na falha tratada, sem manifesto persistente no Destino.
- Após uma Exportação JPEG ou PNG do Álbum inteiro, confirmada para sobrescrita e concluída com sucesso, arquivos órfãos do mesmo Nome e extensão são removidos pela convenção exata.
- A limpeza não cria manifesto, não ocorre em Exportação parcial e só ocorre depois da publicação bem-sucedida de todo o conjunto integral.
- Como os modos `Por lâmina` e `Por página` compartilham o mesmo namespace e não existe manifesto, uma Exportação parcial nunca presume conhecer o modo das saídas preexistentes: ela preserva tudo fora do intervalo e avisa que apenas uma Exportação integral restabelece um conjunto autoritativo completo.

### Operações em lote

- Origens e Destinos de lote aceitam as mesmas formas locais, UNC, mapeadas e verbatim locais/UNC dos demais fluxos. O proprietário reutiliza um contexto transitório durante descoberta e pré-validação e, depois de conhecer as raízes necessárias, congela um único plano imutável de bindings para todos os processos participantes. Uma retomada após reinício cria novo contexto e captura bindings atuais.
- Comparações entre origem e Destino consideram raízes resolvidas e identidade física quando disponível, impedindo que uma unidade mapeada esconda um Destino igual ou interno à origem.
- A Geração de Projetos em lote parte do estado visível integral de um Projeto modelo, inclusive mudanças não salvas, sem salvar ou modificar o modelo.
- A Janela do Projeto modelo abre uma janela dedicada com Projeto modelo somente para consulta, pasta de origem, pasta de destino, quantidade de pastas geradoras e `Cancelar`/`Verificar e gerar`.
- `Verificar e gerar` analisa conflitos e problemas antes de qualquer gravação e abre a Tela de Problemas quando houver pendências.
- Cada Projeto gerado é uma Cópia de Projeto completa e independente, com nova Identidade.
- Todas as Lâminas, composições, Frames, Fotos existentes, padrões, personalizações, travamentos, favoritos e referências do modelo são copiados.
- Uma árvore de pastas de origem é examinada recursivamente. Toda pasta que contenha ao menos uma imagem importável diretamente gera um Projeto com seu próprio nome.
- A busca continua em subpastas mesmo quando a pasta atual gera um Projeto.
- A hierarquia relativa da origem é recriada no destino.
- O arquivo gerado fica diretamente no espelho da pasta-pai e recebe o nome da Pasta de Fotos: `origem/Turma 1/001` produz o Projeto `001` em `destino/Turma 1`, não em uma pasta duplicada `destino/Turma 1/001/001`.
- As imagens diretamente presentes na pasta geradora são acrescentadas à aba `Fotos` do novo Projeto, vinculadas aos originais e não colocadas em Lâminas.
- O destino não pode ser igual à origem nem estar dentro de sua árvore.
- Conflitos de geração são pré-calculados e apresentados na Tela de Problemas, uma linha por Projeto de destino existente.
- Cada conflito oferece `Sobrescrever` ou `Ignorar`; a tela também oferece `Sobrescrever todos` e `Ignorar todos`.
- Um Projeto de destino aberto nunca é incluído em sobrescrita individual ou global. `Sobrescrever` permanece indisponível para sua linha, e ele só pode ser ignorado enquanto continuar aberto.
- A geração só pode continuar quando todos os conflitos tiverem uma decisão e sempre exige clique explícito em `Continuar Geração`; resolver a última linha não inicia a operação automaticamente.
- Uma falha de geração não interrompe ou reverte os demais itens. O resumo final separa sucessos, ignorados e falhas.
- A Exportação em lote encontra recursivamente Projetos e sempre exporta o Álbum inteiro de cada um.
- A Tela de Boas-vindas abre uma janela dedicada de configuração do lote com pasta de origem, Formato, Modo e Destino.
- A janela mostra a quantidade de Projetos descobertos na origem e oferece `Cancelar` e `Verificar e exportar`.
- Formato, modo e destino seguem as opções da Exportação normal, exceto pela ausência de intervalo.
- A Exportação em lote não oferece o slider de qualidade JPEG e sempre codifica esse formato em qualidade máxima.
- `Verificar e exportar` executa a pré-validação antes do Modo de lote exclusivo e abre a Tela de Problemas quando houver pendências.
- A pré-validação do lote usa o mesmo diálogo tabular, oferece `Relinkar` por item e acrescenta `Relinkar todos`.
- `Relinkar todos` solicita uma pasta raiz, procura recursivamente uma pasta com o mesmo Nome exato de cada Projeto e, dentro dela, procura recursivamente o nome e a extensão exatos de cada original ausente.
- Uma única correspondência é aceita automaticamente; nenhuma ou várias correspondências mantêm o problema na tabela.
- Correspondências individuais ou globais do lote formam um mapa temporário daquela execução: não regravam Projetos, não participam de Undo/Redo e são descartadas ao concluir ou cancelar.
- Cada Projeto problemático oferece `Ignorar neste lote`; a ação remove todas as suas pendências da execução sem modificar o arquivo e registra Projeto e motivos no resumo final.
- No lote, `Continuar Exportação` habilita quando todo problema foi corrigido ou seu Projeto foi explicitamente ignorado. A Exportação normal não oferece a ação de ignorar.
- O Modo de lote exclusivo e a janela de progresso começam somente depois dessa etapa de diagnóstico e correção; enquanto a tabela está aberta, `Abrir Projeto` continua disponível.
- A Exportação em lote usa exclusivamente o estado persistido dos arquivos, mesmo quando um Projeto está aberto com mudanças não salvas. Correções criativas feitas por `Abrir Projeto` precisam ser salvas antes de uma nova verificação; o mapa temporário de Religação é a única exceção.
- Imediatamente antes de criar o snapshot imutável de um item, o lote reabre o Projeto pelo núcleo compartilhado, confere sua revisão ou hash persistido contra a versão pré-validada e repete a validação se o arquivo mudou.
- Durante toda a Exportação em lote, uma concessão global exclusiva permanece ativa e o Cache permanece pausado: todas as janelas de Projeto ficam indisponíveis, trabalhos de Cache são interrompidos em ponto seguro e somente a janela de progresso e cancelamento do lote permanece interativa.
- O progresso do lote usa somente a barra geral e `X/Y`, sem expor a tabela de Projetos, quais estão simultaneamente ativos ou contadores separados de concluídos, ignorados e falhas durante o processamento.
- Nenhuma Exportação normal, edição, Salvamento, abertura ou fechamento de Projeto pode começar enquanto o Modo de lote exclusivo estiver ativo.
- Concluir, falhar ou cancelar o lote libera a concessão e a pausa, reabilita todas as janelas e permite retomar os trabalhos de Cache, sem salvar ou alterar automaticamente qualquer Projeto aberto.
- O MVP processa exatamente um Projeto por vez, em ordem determinística, sem Perfil de desempenho, calibração ou paralelismo entre Álbuns. Paralelismo só pode ser reconsiderado depois de medições representativas.
- Por padrão, cada Projeto recebe sua pasta de saída com o próprio Nome ao lado de seu arquivo.
- Em um destino alternativo, a hierarquia relativa dos Projetos é preservada e cada Projeto recebe uma pasta com seu Nome.
- Conflitos de todo o lote são apresentados antes do início com `Sobrescrever todos` ou `Cancelar`.
- Um Projeto inválido pode ser explicitamente ignorado na Tela de Problemas, com seus motivos registrados, sem interromper os demais.
- Cada Álbum usa a mesma preparação e Publicação limitada da Exportação normal. Uma falha pertence ao item corrente e não reverte itens já concluídos.
- O checkpoint do lote é persistido atomicamente em `%LOCALAPPDATA%\MyAlbuns\Recovery\Batches\{batch-id}.json` e registra apenas plano, opções, estados `pendente`, `concluído`, `ignorado` ou `falho` e o item que estava em execução; não contém estado criativo, saída preparada ou manifesto no Destino.
- Se a operação for interrompida, o aplicativo apresenta `Lote interrompido`, com `Retomar` ou `Encerrar`, sem continuar automaticamente. `Retomar` conserva estados terminais e refaz integralmente o item interrompido, nunca do meio de sua publicação.
- `Encerrar` limpa preparações incompletas, preserva os Álbuns publicados e libera as Janelas de Projeto.
- O estado temporário do lote é removido depois de sucesso integral, cancelamento concluído ou `Encerrar`.
- A limpeza de saídas órfãs ocorre isoladamente após o sucesso de cada Projeto, pois todo item do lote representa uma Exportação completa.
- Ao final, sucesso integral mostra uma confirmação curta. Se houver Projetos ignorados ou com falha, a Tela de Problemas apresenta Projeto, Resultado, motivo e ações disponíveis.
- O resumo final informa Projetos exportados, ignorados e com falha, incluindo o motivo de cada problema.

## Testing Decisions

- Os testes devem observar comportamento externo: estado resultante do Projeto, conteúdo persistido, arquivos gerados, mensagens de validação e efeitos visíveis. Não devem depender da futura organização interna do código.
- A fronteira principal de teste deve atravessar os fluxos públicos de edição, persistência, Exportação e operações em lote. Uma entrada combina Projeto persistido, sessão e arquivos externos; a saída observável combina novo estado, artefatos exportados, avisos e erros.
- Como o repositório ainda não possui implementação, não há módulos ou testes anteriores a reutilizar. A arquitetura futura deve preservar essa fronteira de alto nível.
- A criação deve cobrir valores padrão, quantidade mínima, extremidades, cancelamento do diálogo nativo sem arquivo, conversão de Unidade sem mudança física e validações antes da persistência.
- A estrutura do Álbum deve ser verificada por matrizes: quantidade mínima de Lâminas, papéis derivados da ordem, lados ativos, extremidades simples, reordenação, duplicação integral e independente, exclusão e conversão.
- Sangria e Área de segurança devem cobrir Unidades diferentes, valor zero, acumulação, bordas elegíveis, divisão central, página única, validação de áreas positivas e ausência das guias na saída.
- Herança visual e de Frame deve ser testada como transições entre `default`, `custom`, herança parcial, ausência personalizada, restauração e mudança posterior do padrão.
- Frames e Fotos devem ser exercitados em diferentes proporções, rotações, espelhamento, Pan, Zoom, redimensionamento, troca de conteúdo com Foto ou placeholder e cópia/colagem simples ou múltipla na mesma Lâmina, entre Lâminas equivalentes e nos dois sentidos entre Lâmina dupla e Página única, verificando Preenchimento do Frame, deslocamento de colagem limitado, fallback sem deslocamento, mapeamento proporcional, preservação do lado lógico, reutilização dos vínculos, isolamento entre Projetos, ausência de Caixa de seleção e atomicidade do Histórico.
- Layouts devem ser testados por compatibilidade, prioridade, identidade com sequência ordenada, atualização do catálogo ao abrir/focalizar/atualizar, mapeamento pela Pilha visual, preservação de conteúdo e estilo, criação de placeholders, travamento, `Delete` que preserva Frames travados, destravamento e diferenças entre modo normal e Modo de edição.
- Enquanto o algoritmo do Gerador de Layouts estiver adiado, seus testes devem fixar apenas o contrato: sempre produzir ao menos uma opção compatível e respeitar escopo e limites.
- Persistência e identidade devem cobrir Salvar, `Salvar como`, Cópia externa gravável e somente leitura, movimentação, Bloqueio de abertura, bloqueio órfão, isolamento, Undo/Redo em sessão e Recuperação consolidada que reinicia com Histórico vazio.
- Cenários com arquivos temporários reais devem verificar vínculos externos, substituição no mesmo caminho, Arquivo ausente, Arquivo indisponível, religação independente, duplicação entre abas e remoção de itens usados.
- Caminhos devem ser exercitados como local absoluto, UNC, unidade mapeada, verbatim local, verbatim UNC, caminho longo, relativo inválido, namespace de dispositivo, curinga, fluxo alternativo e componente reservado. Os testes também cobrem arquivo no lugar de diretório e o inverso, criação sob pai validado, aliases do mesmo Projeto, `Same`/`Different`/`Indeterminate` com política no chamador, rede indisponível e recuperada, transporte do mesmo plano imutável de bindings por IPC, bindings fixos até o estado terminal, nova captura em `Tentar novamente` ou retomada após reinício e abertura individual de cada original.
- O Painel deve ser testado por ordenação natural, datas do original, arquivos ausentes no fim, busca sem distinção de caixa ou acento, interseção entre busca e filtros, textos temporários independentes por aba, redimensionamento contínuo das miniaturas sem corte, seleção individual, por intervalo e de todos os resultados visíveis, descarte de selecionados ocultados, preservação diante de reordenação, clique direito e remoção contextual por foco, além da persistência somente das preferências previstas sem alterar o Projeto.
- Testes de Exportação devem usar composições canônicas para verificar dimensões físicas, DPI, quantidade e ordem, namespace compartilhado entre modos, recorte central, transparência, Pilha visual, PDF multipágina e leitura dos originais.
- Conflitos de Exportação e geração devem ser pré-calculados, apresentados em conjunto e nunca resultar em substituição ou renomeação silenciosa.
- Nenhum teste de Exportação pode obter sucesso apenas por existir Cache quando o original estiver ausente.
- Publicação deve cobrir falha na preparação, sucesso integral e falha após uma ou mais promoções finais, verificando atomicidade por arquivo quando suportada, aviso de possível mistura, ausência de rollback prometido e proibição de remover órfãos em falha ou intervalo parcial.
- Operações em lote devem cobrir descoberta recursiva, execução estritamente serial, caminho exato no espelho da árvore, conflitos, proteção de Projeto aberto, relinks individuais e globais estritos, revalidação da revisão persistida antes do snapshot, checkpoint por item, retomada que refaz o item interrompido, isolamento de falhas, cópia integral do estado visível do Projeto modelo e importação das novas imagens somente no Painel.
- Namespace, representação reduzida única, metadados, invalidação e políticas de liberação do Cache exigem testes próprios, incluindo a impossibilidade de limpar Cache ativo ao vivo. Formato, resolução, fingerprint e eventual tiling aguardam medições.
- Álbuns longos devem ser testados com virtualização da cena, margem de pré-carga, descarte e reconstrução de texturas, preservando todo o modelo lógico e a latência de navegação.
- O spike arquitetural deve exercitar a pequena interface externa do núcleo e o mesmo conjunto de cenários nas duas topologias, sem acoplar os testes às subdivisões internas, e registrar memória, GPU, processos, abertura, latência do Canvas, propagação de falhas, recuperação e complexidade de IPC/logs.

## Out of Scope

- Impressão e geração de prova.
- Texto, formas, adesivos ou elementos fora da Pilha visual definida.
- Frames não retangulares, rotação de Frame, máscaras livres, cantos arredondados e sombras.
- Comandos automáticos de alinhamento e distribuição de Frames.
- Ângulo da Foto fora do intervalo de `-45°` a `+45°`, espelhamento vertical, brilho, contraste, saturação, opacidade própria da Foto e filtros além de preto e branco.
- Pan, Zoom, recorte ou preservação obrigatória de proporção para imagens de Background e Overlay no comportamento inicial.
- Importação de RAW, PSD, PDF, HEIC ou formatos diferentes de JPG/JPEG, PNG e TIFF/TIF.
- Exportação em formatos diferentes de JPEG, PNG e PDF.
- Unidade de Projeto em pixels.
- Análise ou aviso automático de baixa resolução das imagens.
- Incorporação de mídia ao Projeto.
- Sincronização, herança ou relação de derivação entre Projetos copiados.
- Salvamento automático do conteúdo criativo.
- Sangria externa que amplie a Dimensão da Lâmina.
- Marcas de corte renderizadas na saída.
- Valores de Sangria ou segurança diferentes por borda.
- Exportação parcial em lote.
- Processamento paralelo de Álbuns, calibração automática ou Perfil de desempenho para lote.
- Interface de remapeamento de atalhos ou modificadores de gestos.
- Eleição ou reinício automático do componente global após falha.
- Rollback integral do conjunto durante a Publicação da Exportação.
- Pirâmides de tiles ou múltiplas variantes persistentes por mídia sem necessidade demonstrada por medições.
- Colocação automática nas Lâminas das novas Fotos encontradas pela Geração de Projetos em lote.
- Manifesto ou arquivo auxiliar no Destino da Exportação.

## Further Notes

O [glossário do domínio](../../CONTEXT.md) é normativo somente para o significado dos termos e deve permanecer um glossário. Esta SPEC possui o comportamento funcional, os designs detalham contratos de áreas específicas e as decisões arquiteturais difíceis de reverter estão registradas nos ADRs:

- [Vincular arquivos de mídia em vez de incorporá-los](../adr/0001-vincular-arquivos-externos.md);
- [Atribuir nova identidade a cópias externas](../adr/0002-identificar-copias-externas.md);
- [Limpar saídas órfãs pela convenção de nomes](../adr/0003-limpar-saidas-orfas-pela-nomeacao.md);
- [Manter as margens de acabamento dentro da dimensão exportada](../adr/0004-manter-margens-dentro-da-dimensao-exportada.md);
- [Validar Tauri 2, React/TypeScript e Rust](../adr/0005-adotar-tauri-react-rust.md);
- [Publicar Exportações com transação limitada](../adr/0006-publicar-exportacao-com-transacao-limitada.md);
- [Tratar caminhos Windows como valores nativos e separá-los da identidade física](../adr/0007-tratar-caminhos-windows-e-identidade-fisica.md);
- [Garantir sempre um Layout compatível por arranjo de reserva](../adr/0008-garantir-layout-compativel-por-arranjo-de-reserva.md).

A organização reversível de dados locais e o contrato técnico do Cache estão em [Armazenamento local e Cache](../design/0010-armazenamento-local-e-cache.md). O módulo compartilhado, as formas aceitas e os bindings temporários de cada operação estão em [Resolução e política de caminhos](../design/0011-resolucao-e-politica-de-caminhos.md). A propriedade do estado e os módulos de trabalho estão em [Propriedade de estado e módulos do núcleo](../design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

A limpeza sem manifesto possui uma consequência aceita: um arquivo manual com Nome, extensão e índice indistinguíveis de uma Saída órfã pode ser removido depois da confirmação explícita de sobrescrita.

Neste produto, `Sangria do Projeto` denomina deliberadamente uma faixa interna à dimensão exportada. A terminologia não deve ser reinterpretada como área acrescentada externamente.

A garantia de Publicação é deliberadamente limitada: todas as saídas são preparadas antes, mas cada nome final é promovido separadamente. Uma falha nessa etapa pode deixar mistura entre o conjunto anterior e o novo; não há rollback do conjunto, e Saídas órfãs nunca são removidas nessa falha.

As funcionalidades abaixo permanecem no produto, mas seus detalhes foram deliberadamente adiados e exigem decisões próprias antes da implementação correspondente:

- algoritmo do Gerador de Layouts, diversidade e ordenação de candidatos e distribuição dos Blocos de Frames;
- limite numérico da Mudança dimensional segura e transformação exata de geometria, Pan, Zoom e ponto focal;
- formato e resolução da representação visual reduzida, representação concreta dos identificadores de geração/versão, algoritmo de fingerprint e eventual adoção de tiles depois do spike;
- pipeline de renderização, incluindo qualidade e compressão, perfis de cor, orientação EXIF, tratamento de arquivos TIFF multipágina durante a importação e conversão de medidas físicas em pixels;
- eventual paralelismo entre itens de lote, somente se medições demonstrarem ganho e preservarem o contrato serial observável;
- perfis de hardware mínimo e recomendado e metas quantitativas de desempenho, que serão definidos somente após medições reais do spike;
- comportamento da numeração de arquivos quando uma Exportação ultrapassar o índice `999`;
- formato e extensão do arquivo de Projeto, codificação reversível dos caminhos Windows persistidos e mecanismo interno usado para detectar movimentações e Cópias externas.

O repositório ainda não possui implementação nem convenções de teste estabelecidas. Tauri 2 com React/TypeScript e Rust é a hipótese principal da primeira versão, sujeita a um spike que compare duas topologias: `(A)` um host independente por Projeto e `(B)` um host multiwindow com sessões e Processadores de Imagens isolados. A comparação mede memória, GPU, quantidade de processos, tempo de abertura, latência do Canvas, propagação de falhas, recuperação, IPC e complexidade operacional; a escolha não está congelada pela documentação atual.

As duas alternativas reutilizam um núcleo Rust compartilhado atrás de uma pequena interface externa para carregar, validar, modificar, persistir e criar snapshots do Projeto. Internamente, existe exatamente uma sessão proprietária mutável do estado criativo de cada Projeto, enquanto domínio e persistência conservam responsabilidades próprias. A Janela normal e o lote passam por essa interface; o componente de imagem recebe um snapshot validado e imutável e não interpreta independentemente o documento persistido.

`MyAlbuns.exe` permanece como nome pretendido da experiência global e da Tela de Boas-vindas se a topologia escolhida o permitir sem custo desproporcional. PixiJS sobre WebGL2 continua a hipótese para a prévia interativa, enquanto a Exportação Rust reabre os originais. Windows 10/11 x64 é o escopo inicial, WebGL2 com aceleração de hardware verificável é requisito do editor e WPF/.NET com C# permanece contingência. Formato do arquivo de Projeto, topologia final dos processos e controles concretos da interface ainda deverão ser definidos sem alterar os contratos funcionais desta SPEC.
