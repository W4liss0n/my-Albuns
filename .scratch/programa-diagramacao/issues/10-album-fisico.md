# 10 — Álbum físico

**What to build:** permitir criar e editar a estrutura física completa do Álbum, navegar por Lâminas e Páginas em escala coerente e visualizar corretamente Sangria, segurança e lados desativados.

**Blocked by:** 08 — Esqueleto ponta a ponta.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0004 — margens dentro da dimensão exportada](../../../docs/adr/0004-manter-margens-dentro-da-dimensao-exportada.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md); [criação de Projeto](../../../docs/design/0003-criacao-de-projeto.md).

- [ ] A criação aceita Unidade física, largura e altura da Lâmina, DPI, quantidade de Lâminas, Sangria do Projeto, Área de segurança e opções de extremidades simples.
- [ ] A etapa física só permite avançar quando todos os campos formam uma configuração válida; erros são inline e o primeiro campo inválido recebe foco.
- [ ] O campo de quantidade aparece somente na criação; `Design do Álbum` não permite redefini-lo, e alterações posteriores de quantidade ocorrem apenas por comandos explícitos de adicionar ou excluir Lâminas.
- [ ] O grupo `Estrutura` de `Design do Álbum` controla independentemente o formato da primeira e da última Lâmina, sem controlar sua quantidade.
- [ ] O DPI pode ser alterado em `Design do Álbum > Documento`; a aplicação preserva medidas físicas, geometria e enquadramentos, altera a resolução em pixels e cria uma única ação de Undo/Redo.
- [ ] A dimensão informada pertence à Lâmina; cada Página ocupa metade da largura, sem aumentar tamanho por Sangria ou segurança.
- [ ] O editor permite navegar, excluir e reordenar Lâminas; a exclusão só é aceita enquanto restarem ao menos duas, e a vizinha assume o papel da extremidade removida.
- [ ] O menu `Lâmina` oferece adicionar antes/depois, excluir e converter a extremidade centralizada; o menu de contexto da superfície ou Barra oferece as mesmas ações para o alvo clicado.
- [ ] Desabilitar adição, exclusão, conversão e reordenação no Modo de edição; permitir mudanças de sequência somente no modo normal.
- [ ] Adicionar cria uma Lâmina dupla vazia, sem Frames/Fotos/Layout e com Background/Overlay herdados do Projeto, como uma única ação de Undo/Redo.
- [ ] Permitir inserção externa somente quando a extremidade atual for dupla; desabilitar qualquer inserção que empurre uma Página única para o interior.
- [ ] Recalcular a Numeração e centralizar a nova Lâmina no Canvas sem incluir a posição visual no Histórico.
- [ ] Desabilitar `Excluir` com duas Lâminas; nos demais casos, excluir sem confirmação e restaurar toda a Lâmina/composição por uma única ação de Undo/Redo.
- [ ] Preservar itens de `Fotos` e `Decorativos` e seus arquivos externos, mesmo quando a exclusão remove o último uso.
- [ ] Recalcular papéis/Numeração, centralizar a próxima Lâmina ou a anterior no fim e fechar o Painel de Layouts se apontava para a removida.
- [ ] Reordenar por arraste de uma área livre da Barra: a origem vira espaço reservado, um fantasma acompanha o ponteiro e as Lâminas intermediárias deslizam conforme o espaço muda de posição.
- [ ] Permitir o mesmo arraste pelas miniaturas da Grade de Lâminas; clique sem arraste apenas centraliza, e a grade usa espaço reservado, fantasma e deslocamento segundo a sequência.
- [ ] Durante o arraste na Grade, oferecer rolagem vertical automática progressiva nas bordas e manter o espaço reservado atualizado.
- [ ] Manter a representação oposta estável durante a prévia e sincronizá-la somente depois do commit válido.
- [ ] Oferecer rolagem horizontal automática progressiva nas bordas durante o arraste, atualizando continuamente o espaço reservado.
- [ ] Não alterar a ordem até a soltura válida; confirmar como uma única ação, cancelar por `Esc`/alvo inválido e rejeitar qualquer posição que empurre Página única ao interior.
- [ ] Uma Lâmina de página única permanece presa à extremidade correspondente, e qualquer reordenação direta ou indireta que a empurre para o interior é rejeitada.
- [ ] Lâminas inicial e final podem ser inteiras ou de página única conforme a configuração, e o lado desativado não recebe conteúdo nem interação.
- [ ] No modo normal, a faixa interna de Sangria é ocultada; no Modo de edição, todo o conteúdo e as guias de Sangria e segurança ficam visíveis.
- [ ] Guias são aplicadas apenas às bordas elegíveis da superfície ativa e nunca aparecem na Exportação.
- [ ] Sangria e segurança começam no equivalente físico a `3 mm`, podem ser zeradas independentemente, acumulam-se nas bordas elegíveis e são rejeitadas quando eliminam a Área de corte ou a Área de segurança de alguma Página ativa.
- [ ] Alterar Sangria ou segurança posteriormente preserva a composição, atualiza máscara e guias, participa de Undo/Redo e exige Salvamento; as guias nunca restringem o conteúdo.
- [ ] Os campos de Sangria e segurança usam a Unidade do Projeto, confirmam por `Enter` ou perda de foco e rejeitam valores inválidos inline sem modal ou botão `Aplicar`.
- [ ] Alterações estruturais iniciadas em `Design do Álbum` são pré-validadas, usam `Aplicar` e apresentam o impacto antes de modificar atomicamente o Projeto; não salvam o arquivo.
- [ ] Estrutura, dimensões e ordem participam de Undo/Redo, sobrevivem a `Salvar`/reabrir e são respeitadas ao exportar a superfície ativa.
- [ ] Matrizes automatizadas cobrem quantidade mínima, extremidades, divisão central, Unidade e valores zero de acabamento.
