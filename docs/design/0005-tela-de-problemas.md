---
status: accepted
document: design
updated: 2026-09-24
---

# Tela de Problemas

## Objetivo

A Tela de Problemas é uma superfície reutilizável para diferentes categorias de problema do aplicativo. Ela não pertence exclusivamente à Exportação: cada fluxo pode abri-la com as linhas e ações pertinentes ao seu contexto.

Na primeira versão, ela não possui item permanente em `Exibir` nem outro comando para abertura manual. A tela surge automaticamente quando um fluxo encontra problemas e já começa filtrada para aquela operação.

Ela não bloqueia o acesso necessário às Janelas de Projeto. A operação que a originou permanece pendente enquanto o usuário abre e corrige Projetos, sem iniciar processamento final em segundo plano.

## Lista

A tela apresenta uma lista, sem colunas nem caixa em volta. Cada entrada mostra
o objeto afetado em destaque, os problemas logo abaixo e suas ações compactas à
direita. As entradas vão de uma borda à outra da janela, separadas por uma linha
fina. O objeto de cada entrada depende do fluxo:

- em operações com vários Projetos, como a Exportação em lote e a Geração em
  lote, há **uma entrada por Projeto**, com o Nome do Projeto como título e o
  caminho apenas no tooltip. Todos os problemas daquele Projeto ficam juntos na
  mesma entrada, e problemas de imagem do mesmo tipo formam uma única linha:
  `Imagem ausente: 001.jpg`, `2 imagens ausentes: 001.jpg, 014.jpg` ou, com mais
  de três arquivos, `14 imagens ausentes: 001.jpg, 003.jpg, 007.jpg e mais 11`,
  com a lista completa no tooltip. Imagens ausentes e indisponíveis ficam em
  linhas separadas, porque têm ações diferentes; outros problemas conservam sua
  mensagem;
- em operações de um único Projeto, como a Exportação normal e a importação no
  Painel, todas as entradas pertencem ao mesmo Projeto. Por isso há **uma
  entrada por arquivo ou posição**, com o nome do arquivo (ou `Lâmina 02,
  posição 3`) como título, o problema abaixo (`Arquivo ausente.`) e o Nome do
  Projeto apenas no tooltip.

O wireframe abaixo exemplifica a variante de Exportação em lote; ações como
`Ignorar` e `Relinkar todos` não aparecem em todos os contextos.

```text
┌──────────────────────────────────────────────────────────────────┐
│                       Exportação em lote                      ✕  │
├──────────────────────────────────────────────────────────────────┤
│  Problemas na exportação                                         │
│  Resolve ou ignore os projetos abaixo para continuar.            │
├──────────────────────────────────────────────────────────────────┤
│  Álbum 001                  [Abrir projeto] [Ignorar neste lote] │
│  Preencha os quadros vazios e salve o projeto.                   │
├──────────────────────────────────────────────────────────────────┤
│  Álbum 002       [Abrir projeto] [Localizar imagens…] [Ignorar…] │
│  4 imagens ausentes: 001.jpg, 003.jpg, 007.jpg e mais 1          │
├──────────────────────────────────────────────────────────────────┤
│  Localizar todas as imagens…     Fechar   Continuar exportação   │
└──────────────────────────────────────────────────────────────────┘
```

Ações que valem para a lista inteira, como `Substituir todos` e `Ignorar todos`
na Geração, ficam à direita da descrição. No rodapé, em faixa de tom, as ações
secundárias ficam à esquerda e `Fechar` fica à direita, junto da ação principal
quando houver. Entradas sem correção possível dentro daquele resultado não têm
ações. A decisão de 24/09/2026, validada em protótipo, substitui a tabela com
colunas `Projeto`, `Problema` e `Ações`; a barra da janela segue o padrão da
janela que a apresenta, sem a marca.

## Importação no Painel

Uma importação com vários arquivos possui sucesso parcial. Arquivos válidos são acrescentados ao Painel mesmo quando outros forem inválidos, corrompidos ou incompatíveis; uma falha não reverte os itens já aceitos. Caminhos já presentes na aba seguem a regra normal de duplicata e não aparecem como falha.

Se houver alguma rejeição, a Tela de Problemas é aberta ao término com uma
entrada por arquivo: o nome do arquivo como título e o motivo abaixo.

Essa visão é um resultado da operação já concluída. Fechá-la não desfaz os itens importados com sucesso.

## Visão de Exportação

Na Exportação, `Abrir Projeto` é a ação de um problema com Frame placeholder. `Relinkar` é a ação de um Arquivo original cuja ausência foi confirmada sob uma origem acessível. Arquivo indisponível preserva seu vínculo e oferece `Tentar novamente`; essa ação cria nova tentativa de acesso sem Undo/Redo, Religação ou Salvamento.

Depois de qualquer correção, a validação é executada novamente e a lista remove ou atualiza os problemas resolvidos.

Na Exportação normal, a lista permanece aberta somente enquanto houver problemas. Ao resolver a última pendência por `Relinkar` ou `Tentar novamente`, a janela fecha e a tentativa continua automaticamente para a mesma seleção, usando o estado visível atualizado. Não existe etapa vazia de confirmação nem botão `Continuar Exportação` nesse fluxo. A janela fecha antes de qualquer seletor de destino da continuação.

Se o Original foi recuperado, mas houve falha na preparação de sua prévia, o diagnóstico de processamento ainda é apresentado por arquivo, com o motivo. Fechá-lo encerra a tentativa pendente e preserva o vínculo corrigido; a retomada automática exige ausência também dessas falhas reais.

Na Exportação em lote, `Continuar Exportação` permanece desabilitado enquanto houver qualquer pendência sem decisão. Ao corrigir a última linha ou ignorar explicitamente seu Projeto, o botão é habilitado e o usuário confirma a continuação.

Fechar ou cancelar a Tela de Problemas encerra a tentativa de Exportação pendente. Relinks realizados na sessão de um Projeto individual permanecem como alterações não salvas. O mapa temporário produzido para um lote é descartado. Esse mapa funcional não é o contexto técnico da [política de caminhos](0011-resolucao-e-politica-de-caminhos.md), que reutiliza fatos de raízes somente dentro de cada tentativa de processamento.

## Exportação individual

Na Exportação normal, `Relinkar` abre um seletor de pasta para o usuário informar onde estão as Fotos daquele Projeto. A busca considera somente os arquivos diretamente nessa pasta, sem pesquisar subpastas, e exige o nome e a extensão exatos do Arquivo ausente.

Somente uma correspondência única é aceita. Nenhuma ou várias mantêm a pendência; uma correspondência aceita atualiza a referência na sessão aberta. A mudança:

- participa de Undo/Redo;
- marca o Projeto como alterado;
- não executa Salvamento automático;
- pode ser usada imediatamente pela Exportação normal, que trabalha com o estado visível.

## Exportação em lote

No lote, `Relinkar` em uma linha solicita a pasta das Fotos daquele Projeto e mantém a busca recursiva, exata e não ambígua. A correspondência integra somente o mapa temporário do lote e não atualiza o arquivo persistido.

O contexto do lote também acrescenta a ação global `Relinkar todos`. Ela solicita uma pasta raiz onde se encontram as pastas de Fotos dos Projetos.

Cada Projeto problemático também oferece `Ignorar neste lote`. A escolha retira explicitamente aquele Projeto da execução e resolve de uma vez todos os seus problemas na lista, sem alterar seu arquivo. O Projeto e os motivos permanecem registrados como ignorados no resumo final.

Para cada Projeto, o programa procura recursivamente, sob a raiz escolhida, uma pasta cujo nome seja exatamente igual ao Nome do Projeto. Dentro dela, procura também recursivamente o nome e a extensão exatos de cada Arquivo ausente.

Somente uma correspondência única é aceita automaticamente. Se não houver resultado ou houver mais de um, o item permanece na lista para evitar uma ligação ambígua. Uma Foto encontrada para um álbum nunca é usada para resolver automaticamente outro álbum.

Essas associações formam um mapa temporário da execução. Elas:

- não regravam arquivos de Projeto;
- não participam de Undo/Redo;
- não interferem em Projetos abertos ou em mudanças não salvas;
- são descartadas quando o lote termina ou é cancelado.

A lista pertence à preparação do lote. O Modo de lote exclusivo e o progresso começam somente depois dessa etapa, permitindo que `Abrir Projeto` funcione enquanto os problemas estão sendo avaliados. Correções criativas feitas no editor, como preencher placeholders, precisam ser salvas antes de uma nova verificação, porque o lote reabre a versão persistida.

Para o lote, `Continuar Exportação` é habilitado quando cada problema tiver sido corrigido ou seu Projeto tiver sido explicitamente ignorado. A Exportação normal retoma automaticamente ao resolver suas pendências e não oferece `Ignorar neste lote`.

## Geração de Projetos em lote

Quando um caminho planejado já contém um Projeto, a Tela de Problemas apresenta uma linha de conflito para esse destino.

Cada linha oferece:

- `Sobrescrever`, para substituir somente aquele Projeto;
- `Ignorar`, para manter o Projeto existente e retirar aquele item da geração.

O contexto também oferece `Sobrescrever todos` e `Ignorar todos`. As ações globais aplicam a mesma decisão a todos os itens compatíveis, sem esconder os casos que não puderem recebê-la.

Se o Projeto de destino estiver aberto, `Sobrescrever` fica indisponível para aquela linha. `Sobrescrever todos` também não o inclui. Enquanto ele permanecer aberto, a única decisão possível para esse item é `Ignorar`; para substituí-lo, o usuário precisa fechá-lo e executar uma nova verificação.

A geração permanece pendente até que cada conflito tenha uma decisão. Mesmo depois da última decisão, nenhum arquivo é criado ou sobrescrito automaticamente: o usuário ainda confirma explicitamente `Continuar Geração`.

## Resultado de operações

A mesma Tela de Problemas apresenta resultados que exigem atenção após uma operação. Nesse contexto, cada entrada identifica no mínimo o Projeto, o resultado e as ações disponíveis, incluindo o motivo de itens ignorados ou com falha.

Ela só é aberta quando há algo a tratar. Sucesso integral não abre uma lista vazia: na importação de Fotos, o progresso fecha diretamente; nas demais operações, usa uma confirmação curta.
