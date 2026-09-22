---
status: accepted
document: design
date: 2026-09-22
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
- **Escrever pouco e com clareza.** Preferir português simples e orientações
  úteis. Evitar termos técnicos, instruções repetidas e descrições óbvias da ação.
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
| Prévia dos projetos recentes | Primeira lâmina salva, inteira e proporcional, sobre superfície clara com limite fino do papel e sombra curta. Álbum fechado neutro durante a consulta ou indisponibilidade. Cartão de 160 px, miniatura de 126 px e faixa inferior de 32 px. Nome e data compacta na mesma linha; nome longo usa reticências. Hover na data mostra o horário e fecha ao sair dela; hover no nome truncado mostra o nome completo. Teclado continua no cartão. Sem data conhecida, só nome. | [0002](../docs/design/0002-tela-de-boas-vindas.md) |
| Favoritos na tela de boas-vindas | Mesmo cartão em grupo anterior aos recentes, sem duplicatas. Estrela de 16 px em botão de 28 px no canto superior direito; preenchida e sempre visível quando ligada, discreta no hover do cartão ou foco visível quando desligada. Cinza quente secundário em repouso; grafite no hover da estrela ou foco visível, sem fundo no botão. A vazada cobre a imagem dentro da silhueta e ambas têm separação clara fina sobre a miniatura. O rodapé não muda. | [0002](../docs/design/0002-tela-de-boas-vindas.md) |
| Validação de campos | Tooltip compartilhado, fora do fluxo, sem deslocar os controles e com descrição acessível. | [0040](../docs/design/0040-fatos-do-core-e-controles-do-editor.md) |

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
