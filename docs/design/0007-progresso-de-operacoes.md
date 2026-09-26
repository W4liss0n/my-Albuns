---
status: accepted
document: design
updated: 2026-09-24
---

# Progresso de operações

## Objetivo

O aplicativo reutiliza uma única representação simples para operações que precisam mostrar progresso. O componente não expõe a organização interna em processos, threads, filas ou trabalhos simultâneos.

Desde 24/09/2026, por decisão do autor validada em protótipo, a barra da janela
mostra o nome do fluxo que abriu o progresso (`Exportar`, `Exportação em lote`,
`Gerar projetos em lote`), sem a marca. O corpo usa margens de 24 pixels e o
rodapé com `Cancelar` fica em uma faixa de tom, como nas demais janelas.

Cada tentativa fornece seu próprio `ProgressSink` ao componente. A janela somente apresenta os eventos recebidos: não possui o trabalho, a exclusividade global ou o token de cancelamento e não mantém um serviço global de progresso.

A representação também não decide o ciclo de vida da janela solicitante. Ela segue a política do fluxo que a abriu: o padrão de [diálogo pertencente](0001-estrutura-da-janela-do-projeto.md#diálogos-pertencentes-a-uma-janela) preserva a proprietária visível e bloqueada, e somente a [transição de abertura de um Projeto existente](0002-tela-de-boas-vindas.md#transição-de-abertura) a substitui temporariamente.

## Progresso determinado

Quando a operação conhece um total confiável, a janela mostra:

- o título da operação e, somente quando necessário, uma linha curta com o estado atual;
- uma única barra de progresso geral;
- a porcentagem concluída à esquerda, abaixo da barra;
- a contagem `X de Y` à direita, na mesma linha, com a tipografia de apoio do componente.

```text
┌──────────────────────────────────────────────┐
│  Exportando                                  │
│                                              │
│  ███████████████░░░░░░░░░░░░                │
│  43%                           18 de 42       │
└──────────────────────────────────────────────┘
```

Esse é o padrão de todos os diálogos determinados: abertura e geração de Cache,
importação, processamento de imagens e exportações normal e em lote. O componente
compartilhado calcula e apresenta `X de Y` automaticamente a partir das unidades
concluídas e do total. A contagem não ocupa uma linha acima da barra.

Quando as unidades da operação diferem da escala da porcentagem geral, o fluxo
fornece apenas o rótulo da contagem, como `8 lâminas de 18` ou `7 álbuns de 18`.
Posição, tipografia e espaçamento permanecem sob responsabilidade do mesmo
componente. Não há apresentação alternativa para o lote, nem estimativa de tempo
nesse espaço. Uma linha de estado opcional descreve a operação sem repetir sua
contagem ou seu título.

Na Exportação, o refinamento vigente mostra `Exportando`, a contagem
`X lâminas de Y` ou `X páginas de Y`, barra geral, porcentagem e cancelamento
quando disponível. A contagem acompanha as unidades preparadas, inclusive as
páginas internas de um PDF. Não apresenta carregamento dos
Originais, composição, codificação, verificação ou publicação como etapas visíveis.
A porcentagem é contínua entre essas etapas e ao retomar uma tentativa viva.
O lote conserva a contagem compacta `X álbuns de Y`. Nas duas exportações, a contagem ocupa o espaço à direita abaixo da barra, na mesma linha da porcentagem e com a mesma tipografia da geração de Cache. O componente compartilhado fornece essa apresentação; não há uma linha de contagem acima da barra.

O `ExportPipeline` possui a conversão das etapas em percentual geral por Álbum.
A Exportação normal recebe esse valor pelo contrato de eventos, e o lote agrega
o mesmo cálculo entre seus Itens. Carregamento ocupa 0–10%, composição e
codificação 10–75%, verificação 75–85% e publicação 85–99%. As etapas continuam
internas; a interface apresenta somente o progresso geral.

Cada fluxo preserva o maior percentual apresentado durante a tentativa viva,
inclusive ao retomar, e possui sua conclusão terminal. A Exportação normal
converte o valor recebido em inteiro ao apresentar o diálogo; o lote mantém o
Álbum em até 99% até registrar seu resultado, quando pode contar o Item concluído.
As contagens de Unidades de Exportação e de Álbuns permanecem independentes
desse percentual.

O processamento de imagens usa `Processando Imagens` e `X de Y` em qualquer
ação explícita que precise preparar imagens do Projeto. Uma unidade inclui a
validação da origem e a conclusão do Cache, inclusive seu reuso quando válido.
Uma imagem só avança o contador depois dessa preparação ou do registro de um
problema; leitura e Cache não reiniciam a barra em fases separadas. Mudanças de
área visível não cancelam o trabalho necessário à ação. Atualizações automáticas
da origem continuam em segundo plano e preservam a prévia anterior até a troca.

A contagem avança assim que a prévia reduzida está preparada e validada. A
publicação conjunta do índice do Cache permanece dentro da operação aguardada,
sem contar novamente as imagens; uma falha nessa publicação continua sendo
apresentada no resultado. Isso permite atualizar a barra durante a preparação,
em vez de concentrar todos os avanços no fim do lote.

Adicionar uma Foto ou aplicar um Decorativo já importado à Lâmina não inicia
outra preparação, não abre progresso e não aguarda o Cache. O gesto atualiza
a composição imediatamente e reutiliza a prévia disponível. Se uma prévia
precisar ser recuperada, isso acontece sob demanda, em segundo plano. A mesma
regra vale para desfazer e refazer essas aplicações. Novos vínculos, inclusive
os restaurados no catálogo, continuam seguindo o processamento de imagens.

Na abertura, o diálogo começa com `Preparando a Janela do Projeto…`. Quando
existe Cache a reconstruir, a mesma janela passa para `Preparando imagens`,
com barra determinada, porcentagem e contagem `X de Y` das imagens que precisam
ser preparadas. O espaço da porcentagem fica reservado desde o início para
evitar alteração do tamanho da janela nessa transição. A conclusão da contagem
não libera o editor antes da entrega das miniaturas visíveis.

Os diálogos de progresso do Global usam um grupo de processos WebView2 próprio,
com perfil em `State/WebView2/global-progress` e renderização sem aceleração de
GPU para essa superfície simples de texto e barra. O Canvas do Projeto mantém
sua renderização acelerada em outro grupo. Assim, uma falha no navegador
da Tela de Boas-vindas ou de Configurações não apaga o acompanhamento enquanto
o Host prepara as imagens. A propriedade da janela, seu tamanho e a transição
para o editor continuam seguindo o fluxo descrito acima. Falhas de processos
WebView2 são registradas nos logs com a superfície e o código de saída.

Na importação, o Painel de imagens mantém o conjunto anterior durante todo o
lote. Novos cartões e sua contagem aparecem juntos quando o processamento
termina, inclusive quando há problemas a apresentar. Notificações do Monitor de
Arquivos durante a ação não antecipam espaços vazios na grade.

A conclusão da importação inclui a entrega conjunta das prévias ao Painel.
O progresso permanece aberto enquanto a interface carrega as miniaturas que
ficarão visíveis, usando os filtros, a ordem, a rolagem e o tamanho atuais.
Ao inserir o lote ordenado, a grade conserva a posição de rolagem em pixels,
sem deslocamento automático do navegador para acompanhar um cartão anterior.
Resultados parciais de prévias não liberam cartões individualmente. Falhas
aparecem como prévias indisponíveis e seguem para o resultado da operação.
As demais miniaturas continuam sob demanda; essa entrega não exige manter
todo o catálogo decodificado na memória.

## Progresso em lote

Na Exportação em lote do MVP, o refinamento específico mostra `Exportando`, a barra geral determinada, o percentual e a contagem `X álbuns de Y`. Não lista o Projeto atual, estado individual ou fila. `Cancelar` é a única ação.

O lote usa a mesma apresentação determinada das demais operações, com a contagem
de álbuns no campo inferior direito. Não acrescenta resumo da fila ou estado do
item atual.

## Cancelamento

`Cancelar` é a única ação opcional da janela. Ele aparece somente quando a operação oferece cancelamento seguro, como nas Exportações normal e em lote.

Uma operação não cancelável não mostra botão desabilitado nem espaço reservado para essa ação.

## Progresso indeterminado

Quando não existe um total confiável, a mesma barra usa uma animação indeterminada contínua. A contagem `X/Y` é omitida por completo, em vez de mostrar valores artificiais.

```text
┌──────────────────────────────────────────────┐
│  Analisando                                  │
│                                              │
│       ███████                                │
└──────────────────────────────────────────────┘
```

## Simplificação

A espera por uma ação em um diálogo de decisão não acrescenta linhas como
`Concluindo…`, `Verificando…` ou `Procurando…` ao corpo nem substitui a descrição
por esses textos. Recuperação de sessão, Cópia externa e resolução de Arquivos
da Exportação mantêm seu conteúdo enquanto as ações ficam desabilitadas.
Erros e resultados continuam apresentados pelo fluxo correspondente; operações
com progresso próprio continuam usando a representação geral de progresso.

A janela não mostra:

- tabela de Projetos ou arquivos;
- múltiplas barras;
- nomes de trabalhos executados em paralelo além do único item atual;
- histórico ou lista item a item de concluídos, ignorados ou com falha;
- detalhes técnicos de processos ou threads.

A Exportação em lote usa o modo de lote com seu progresso geral e processa um
Projeto por vez no MVP. `X` avança somente quando o item alcança estado
concluído, ignorado ou falho; o componente não expõe o Checkpoint do lote.

A Limpeza total do Cache pode reutilizar o mesmo componente quando executada sem Projeto ou Processador ativo, inclusive na inicialização agendada. Ela não pausa Janelas ativas nem remove Cache em uso no MVP.

## Conclusão

A janela de progresso nunca se converte em relatório.

- Sucesso integral fecha a janela e mostra uma confirmação curta.
- No processamento de imagens, sucesso integral fecha diretamente o progresso e devolve o foco à origem da ação, sem diálogo de confirmação nem mensagem ao lado de Importar. A mesma regra vale quando todas as Fotos selecionadas já estão vinculadas. Problemas de Cache preservam imagens válidas já vinculadas e são apresentados junto dos arquivos rejeitados.
- Itens ignorados ou com falha fecham o progresso e abrem a [Tela de Problemas](0005-tela-de-problemas.md) no contexto de resultado.
- RAM física livre reduz a concorrência e não bloqueia, por si só, um trabalho individual. Quando falta capacidade de commit e não existe processamento ativo que possa liberá-la, o progresso termina com um único aviso operacional, sem listar cada imagem como rejeitada nem preencher o total das imagens não processadas. O resultado parcial preserva as imagens já validadas; uma nova execução de Importar reaplica a regra normal de reimportação, sem duplicar vínculos. A política e a apresentação de problemas reais coexistentes estão detalhadas na [admissão por memória](0032-revisao-da-admissao-por-memoria.md).
