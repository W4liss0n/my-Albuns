---
status: accepted
document: design
date: 2026-09-22
updated: 2026-09-24
---

# Direção visual do MyAlbuns

Preferências aprovadas pelo autor para orientar futuras mudanças na interface.
O visual deve ser **integrado, discreto, coerente e bem alinhado**. Cada controle
deve fazer sentido no conjunto e deixar claro como pode ser usado. A fotografia
e a lâmina são o foco; os controles ajudam a trabalhar sem disputar atenção.

## Princípios permanentes

- **Integrar ao contexto.** Compor os controles como parte do painel. Evitar
  caixas isoladas, aparência genérica de formulário e novos blocos sem função.
- **Destacar com propósito.** Preferir mudanças suaves de tom e contraste.
  Evitar contornos chamativos, sombras fortes, preenchimentos saturados e
  efeitos decorativos que não ajudem a entender a interação.
- **Alinhar de verdade.** Manter colunas, linhas de base, larguras e espaçamentos
  consistentes entre rótulos, valores, unidades e ações. Agrupar ações relacionadas
  para que não pareçam controles soltos.
- **Reutilizar antes de criar.** Procurar componentes e tokens compartilhados.
  O mesmo tipo de controle deve ter a mesma aparência e comportamento nas telas
  em que aparece. Respeitar o escopo: um padrão de entrada não redefine botões.
- **Ser discreto e compreensível.** Preservar a indicação de que algo é editável,
  o foco de teclado e as diferenças entre ligado, desligado, misto, bloqueado e
  erro. Discrição não significa esconder estados ou reduzir a legibilidade.
- **Manter a composição estável.** Não inserir dicas, erros ou textos transitórios
  desnecessários que aumentem diálogos ou desloquem itens. Usar o tooltip
  compartilhado para validação de campos e ajuda breve, com acesso por teclado.
  Informações essenciais e progresso necessário continuam disponíveis.
  No visualizador, trocar para a correção de olhos mantém a foto carregada até
  a nova área estar pronta; voltar ao modo normal também preserva essa
  continuidade, sem uma piscada ou animação de desaparecimento. Avançar ou
  voltar imagens, inclusive a referência, segue o mesmo princípio: manter a
  foto exibida até a próxima estar pronta, com nome coerente e sem permitir
  ações sobre a foto anterior como se fosse a nova. Os controles do visualizador
  também conservam posição, foco e tom no carregamento transitório: bloquear
  a ação sem desmontar, piscar ou acrescentar spinner. Erros definitivos
  continuam distinguíveis pelo estado desabilitado.
- **Escrever pouco e com clareza.** Preferir português simples e orientações
  úteis, seguindo o [guia de escrita aceito](../docs/design/0043-plano-de-simplificacao-dos-textos.md#guia-curto-de-escrita)
  e sua [pesquisa](../docs/research/2026-09-18-padroes-de-escrita-para-interfaces.md).
  O padrão vale também para funcionalidades novas, tooltips, nomes acessíveis
  e erros recebidos do Rust. Orientar apenas ações disponíveis e descrever a
  consequência real, sem expor diagnóstico técnico bruto.
- **Simplificar conforme a função.** Usar ícones com tooltip e nome acessível nos
  controles compactos em que esse padrão foi aprovado. Manter texto quando ele
  ajuda a entender a ação; não transformar todos os botões em ícones.

## Superfícies, hierarquia e medidas

A base é a [referência visual vigente](../docs/references/ui-programa-diagramacao/README.md),
refinada pelas decisões aceitas de design. A paleta usa superfícies claras e
neutros quentes, texto em grafite e bordas discretas. Separação vem primeiro da
composição, do espaçamento e de diferenças suaves de superfície. Sombras seguem
o papel já definido para cada camada, como menus e diálogos.

Usar os tokens de [tema](../src/ui/theme.css), sem recriar uma paleta paralela:

- Espaçamento com base de 4 px: 4, 8, 12, 16, 24 e 32 px.
- Altura compacta de 28 px e regular de 31 px; cantos de 4 ou 6 px conforme o
  componente. Preservar as medidas específicas já estabelecidas nas composições.
- Hierarquia compacta existente: apoio de 11 px, rótulo de 11,5 px, controle de
  12,5 px e título de painel de 13 px. Agrupamento, peso e tom complementam o
  tamanho; não introduzir outra escala tipográfica a cada painel.
- O azul continua tendo função nas seleções e ações que já o utilizam.
  A rejeição do azul nos alternadores de efeito e do contorno no hover da prévia
  não elimina os sinais necessários de seleção, foco ou ação principal.

## Padrões reutilizáveis já aceitos

| Controle e escopo | Padrão | Decisão detalhada |
| --- | --- | --- |
| Entradas integradas do painel contextual | `TextInput` com `appearance="integrated"`: fundo transparente, sublinhado discreto, hover neutro e foco em grafite. `UnitInput` de 92 px com unidade fixa dentro do campo. Erro no sublinhado e tooltip. | [0048](../docs/design/0048-painel-contextual-do-quadro.md) |
| Alternadores compactos de efeito | `PropertyToggle` de 28 × 28 px, ícone de 16 px e cantos de 4 px. Ligado com cinza quente, grafite e leve sombra interna; sem preenchimento azul ou checkbox separado. | [0048](../docs/design/0048-painel-contextual-do-quadro.md) |
| Giro e espelhamento do quadro | Grupo de 92 px com divisória discreta; espaçamento de 12 px entre ajustes. Sem ação separada de restaurar giro: continuar girando completa a volta. | [0048](../docs/design/0048-painel-contextual-do-quadro.md) |
| Prévia de páginas e hover | `VisualScopePreview` muda o tom da página inteira com `--ui-text-muted` a 12%, sem contorno, sombra ou recuo. Conteúdo geral ou lâmina real no mesmo componente; seleção e foco permanecem distinguíveis. | [0049](../docs/design/0049-previa-compartilhada-de-escopo.md) |
| Títulos de seção de conteúdo | `.ui-section-heading`: texto em frase, 13 px, peso 600, cinza secundário, com contagem opcional de 11 px em tom discreto e oculta para leitores de tela. Organiza grupos de conteúdo, como os Projetos da tela de boas-vindas. O `.ui-section-eyebrow` continua nos rótulos de escopo dos painéis. | [0002](../docs/design/0002-tela-de-boas-vindas.md#refinamento-de-24092026) |
| Prévia dos projetos recentes | Primeira lâmina salva, inteira e proporcional, sobre superfície clara com limite fino do papel e sombra curta. Álbum fechado neutro durante a consulta ou indisponibilidade. Grade com colunas de pelo menos 184 px e intervalo de 16 px; miniatura de 136 px com lâmina de até 104 px e faixa inferior de 54 px, sem seta. Nome em destaque e, abaixo, a última abertura completa (“Aberto hoje às 14:30”), sem tooltip na data; nome longo usa reticências e mostra o nome completo no tooltip. Teclado continua no cartão, com borda grafite no foco. Sem data conhecida, só nome, na mesma altura. | [0002](../docs/design/0002-tela-de-boas-vindas.md) |
| Favoritos na tela de boas-vindas | Mesmo cartão em grupo anterior aos recentes, sem duplicatas. Estrela de 16 px em botão de 28 px, a 4 px do canto superior direito; preenchida e sempre visível quando ligada, discreta no hover do cartão ou foco visível quando desligada. Cinza quente secundário em repouso; grafite no hover da estrela ou foco visível, sem fundo no botão. A vazada cobre a imagem dentro da silhueta e ambas têm separação clara fina sobre a miniatura. O rodapé não muda. | [0002](../docs/design/0002-tela-de-boas-vindas.md) |
| Painel de entrada da tela de boas-vindas | Painel de 320 px alinhado ao topo com margem de 32 px: marca, ações principais de 40 px com cantos de 4 px, grupo secundário separado por borda e versão discreta no pé. Área de Projetos sem título de página; os grupos começam na altura da marca. | [0002](../docs/design/0002-tela-de-boas-vindas.md#refinamento-de-24092026) |
| Ações secundárias da tela de boas-vindas | `Configurações…` e `Exportação em lote` têm ícone de 14 px, altura de 28 px, fundo transparente e posição fixa. Texto e ícone vão de cinza secundário a grafite no hover e foco visível, sem sublinhado. Ação desabilitada não reage. | [0002](../docs/design/0002-tela-de-boas-vindas.md) |
| Validação de campos | Tooltip compartilhado, fora do fluxo, sem deslocar os controles e com descrição acessível. | [0040](../docs/design/0040-fatos-do-core-e-controles-do-editor.md) |
| Erros de operação nas Configurações | Sem caixa de aviso. A mensagem abre o tooltip de validação compartilhado junto do controle cuja ação falhou; fecha ao clicar fora, reabre com foco ou clique e mantém o controle marcado como inválido e anunciado enquanto o erro vale. | [0009](../docs/design/0009-configuracoes-do-aplicativo.md#outros) |
| Diálogo Exportar | Barra só com `Exportar`, sem a marca. Blocos com títulos curtos de 13 px separados apenas por espaço, sem faixas nem linhas entre eles. Controles de 31 px; só `Exportar` é ação principal. `Exportar como páginas simples` na linha do intervalo, encostada à direita e alinhada com `Escolher…` e `Exportar`. Rodapé em faixa de tom com o resumo, que dá lugar à mensagem da verificação em vermelho, sem caixa. | [0004](../docs/design/0004-exportacao-normal.md#refinamento-de-24092026) |
| Exportação em lote | Mesmo padrão do Diálogo Exportar: barra só com o nome da janela, títulos curtos separados por espaço, controles de 31 px, rodapé em faixa de tom. O modo usa `Exportar como páginas simples` na linha do formato, à direita; `Outra pasta` tem campo e `Escolher…` na mesma linha. | [0006](../docs/design/0006-configuracao-da-exportacao-em-lote.md#refinamento-de-24092026) |
| Gerar projetos em lote | Mesmo padrão do Diálogo Exportar. O modelo é informação: bloco `Modelo` com o nome e uma frase curta, sem caixa nem ícone. Os `Escolher…` são ações comuns; só `Verificar e gerar` é principal. | [0008](../docs/design/0008-configuracao-da-geracao-em-lote.md) |
| Tela de Problemas | Lista sem colunas nem caixa: título do item em destaque, problemas abaixo e ações compactas de 28 px à direita; entradas de ponta a ponta com uma linha fina entre elas. Uma entrada por Projeto em lote e geração, com problemas de imagem do mesmo tipo agrupados (“2 imagens ausentes: …”, “… e mais N” com a lista no tooltip) e o caminho só no tooltip; uma entrada por arquivo ou posição quando todas pertencem ao mesmo Projeto. Ações gerais à direita da descrição; rodapé em faixa de tom com secundárias à esquerda e `Fechar` junto da principal. | [0005](../docs/design/0005-tela-de-problemas.md#lista) |
| Mensagens, confirmações e progresso | `MessageDialog`, `ConfirmationDialog` e `ProgressDialog` sem ícone de tom: título de 13 px e texto dizem o que houve; o título de erro fica em `--ui-danger-text`. Margens de 24 px e rodapé em faixa de tom (`--ui-surface-muted`). Nenhum diálogo mostra a marca: a barra mostra o nome do fluxo (todas as etapas da exportação de um projeto, lote e geração) ou fica sem texto nos diálogos fora de um fluxo nomeado. O botão vermelho de exclusão continua. | [0007](../docs/design/0007-progresso-de-operacoes.md) |
| Painel do Novo projeto | `Modelo inicial` abre o painel nas duas etapas. Configurações usa os grupos e nomes do editor (`Documento`, `Dimensão da lâmina fechada`, `Áreas técnicas`, `Estrutura`) em grade de duas colunas, com campos integrados do editor e títulos de subseção de 11 px em cinza secundário. Fundo e Sobreposição usam a mesma ação compacta `Escolher imagem…`. Título da janela sem a marca; rodapé em faixa com `Cancelar` como botão comum. | [0003](../docs/design/0003-criacao-de-projeto.md#refinamento-de-24092026) |
| Janela de Configurações | Abas em faixa de tom de 40 px; a selecionada usa a superfície do conteúdo, traço azul de 2 px no topo e bordas laterais finas. Cada seção abre com uma faixa de cabeçalho de ponta a ponta (`--ui-panel-surface`), título à esquerda e ação discreta à direita, como no Painel contextual. Sem cartões flutuantes nem ícones decorativos. Valores lidos em rótulo sobre valor monoespaçado; no máximo uma frase curta por seção. Rodapé no mesmo tom das abas. | [0009](../docs/design/0009-configuracoes-do-aplicativo.md#refinamento-de-24092026) |
| Foco nas miniaturas de fotos e decorativos | O foco de teclado usa a borda existente da miniatura em grafite, sem acrescentar contorno externo ou mudar seu tamanho. A seleção continua distinguível. Pressionar Espaço para abrir o visualizador não cria outra caixa em volta da foto. | [0050](../docs/design/0050-visualizador-integrado-de-imagens.md) |
| Seleção e zoom na correção de olhos | Selecionar não amplia sozinho. Roda e teclas de zoom aproximam o rosto selecionado; trocar de rosto já ampliado mantém o zoom e acompanha a seleção. As caixas continuam disponíveis durante o processamento, a prévia e a comparação. Um novo par refaz a correção sobre a foto original, substituindo a prévia anterior. Só o resultado do par atual pode ser salvo. | [0050](../docs/design/0050-visualizador-integrado-de-imagens.md) |
| Visualizador de imagens em janela própria | Janela owned com superfície neutra quente e imagem inteira como foco. No encaixe, imagem proporcional e centralizada com pelo menos 24 px de respiro nas quatro bordas da área de visualização (`--ui-space-5`). A barra da janela mostra somente o nome truncado e os controles nativos. Setas, olhos e ajuste usam ícones em grafite sobre círculos de 36 px com superfície clara translúcida (56% em repouso, 72% no hover), sem sombra. Alvo de 42 px e foco visível preservados. Estados transitórios ficam na área da imagem. Sem barra de ferramentas, contador ou porcentagem. | [0050](../docs/design/0050-visualizador-integrado-de-imagens.md) |
| Correção de olhos no visualizador | Abrir olhos fica no canto superior direito e se transforma em Fechar correção no mesmo ponto. Seu fundo circular se prolonga à esquerda em uma cápsula contínua translúcida com Antes e depois e Salvar, sem discos separados, bordas, divisórias ou sombra; revelação discreta com movimento reduzido respeitado. Usar esta foto e Trocar referência ocupam o mesmo círculo flutuante no centro inferior da metade esquerda, a 16 px da borda, sem rodapé. Usa ImageToolButton (alvo de 42 px, ícones de 18 px, superfície de 36 px de altura, tooltip e foco acessíveis). Imagens inteiras e proporcionais ocupam as duas metades com 24 px de respiro, sem nomes, rótulos ou faixa reservada para ferramentas. Selecionar os rostos no destino e na referência prepara a correção automaticamente. Antes e depois indica seu estado no botão e no tooltip. Salvar abre a confirmação compartilhada Substituir foto original?, centralizada sobre as fotos, com Cancelar em foco; só Substituir original grava o resultado sobre o arquivo original. Esc cancela apenas a confirmação, preserva a prévia e devolve o foco a Salvar. Ao fechar o modo de correção, imagem e navegação continuam disponíveis no visualizador. Rostos recebem caixas de contorno dos pontos detectados, com separação clara e escura fina para fotos claras e escuras, sem numeral nem preenchimento. O selecionado reforça o contorno e ganha um pequeno Check em disco neutro integrado ao canto; caixas acompanham zoom e deslocamento, com aria-pressed. Sem mensagens soltas sobre as imagens: erros e ausência de rosto ficam apenas no tooltip da própria foto afetada, por hover ou foco de teclado, e fecham ao sair dela. Os botões mantêm somente as dicas de suas ações. Sem textos de andamento como Analisando, Preparando ou Aplicando correção; ocupação indicada pelos controles bloqueados e pelo estado acessível. | [0050](../docs/design/0050-visualizador-integrado-de-imagens.md) |

Os padrões de [controles visuais](../docs/design/0044-controles-visuais-compartilhados.md)
e de [formulários e menus](../docs/design/0045-controles-compartilhados-de-formularios-e-menus.md)
complementam essas decisões. Os documentos específicos mantêm os detalhes dos
contratos e dos estados; esta página orienta a direção visual comum.

## Aplicação em mudanças futuras

Consultar esta direção e o componente existente antes de propor outro visual.
Conferir a alteração no painel real, incluindo a prévia compacta quando afetada,
e os estados e escalas pertinentes. Um controle isolado não basta para avaliar
alinhamento, hierarquia e integração com os vizinhos.

Novas orientações explícitas do autor prevalecem sobre decisões anteriores no
escopo indicado. Registrar refinamentos aceitos na fonte correspondente e manter
este resumo atualizado quando o padrão for reutilizável, sem ampliar a mudança
para tipos de controle que não foram pedidos.
