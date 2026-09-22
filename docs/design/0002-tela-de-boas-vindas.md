---
status: accepted
document: design
updated: 2026-09-22
---

# Tela de Boas-vindas

**Referência visual vigente:** [Tela de Boas-vindas](../references/ui-programa-diagramacao/Boas-vindas.dc.html)

## Objetivo

A Tela de Boas-vindas é a superfície principal visível do processo `MyAlbuns.exe`. Ela funciona como ponto de entrada e coordenação do aplicativo, sem incorporar Canvas nem uma sessão criativa mutável de Projeto. Os cartões podem apresentar uma composição imutável da primeira lâmina salva.

## Entradas da primeira versão

A tela oferece:

- `Novo Projeto`, que inicia o fluxo de criação;
- `Abrir Projeto`, que abre o seletor de arquivo do Windows;
- `Projetos recentes`, para localizar e reabrir trabalhos conhecidos;
- `Exportação em lote`, como placeholder visual desabilitado até existir a porta
  que seleciona e processa uma pasta com Projetos persistidos.

`Configurações` e `Ajuda` não aparecem nessa superfície enquanto seus fluxos
globais não estiverem ligados à nova UI. A ausência é intencional: a referência
visual aceita não reserva ações inertes para esses dois destinos.

## Hierarquia visual

`Projetos recentes` é a área dominante e utiliza a maior parte da janela. Ao lado
dela, o painel de entrada dá acesso imediato a:

- `Novo Projeto`;
- `Abrir Projeto`.

Depois de um divisor, `Exportação em lote` aparece como ação visualmente
secundária. Essa hierarquia reproduz a referência aceita sem alterar o conceito
canônico da operação.

```text
┌──────────────────────────────────────────────────────────────────┐
│                            MyAlbuns                               │
├──────────────────────────────────────────┬───────────────────────┤
│                                          │  MyAlbuns              │
│           Projetos recentes              │  Novo Projeto         │
│              cartões                     │  Abrir Projeto        │
│                                          │  ─────────────────    │
│                                          │  Exportação em lote   │
└──────────────────────────────────────────────────────────────────┘
```

As proporções, os espaçamentos e as dimensões seguem a referência visual aceita.

## Projetos recentes

Os Projetos recentes aparecem em uma grade de cartões. Cada cartão reserva uma
capa visual, o Nome do Projeto, a última abertura conhecida e a indicação de abertura.
Clicar em qualquer ponto do cartão abre o Projeto correspondente.

O cartão mostra a **primeira lâmina salva do Projeto**, no lugar da representação
genérica da referência original. Essa decisão do autor, de 22 de setembro de
2026, refina somente o conteúdo da miniatura; a grade, as ações e a hierarquia
da tela permanecem. A lâmina mantém sua proporção e cabe inteira na área
reservada, sem cortar a composição para preencher o cartão.

O frontend recebe o Nome do Projeto, uma Identidade opaca para solicitar sua
reabertura e sua prévia, e o instante da última abertura quando conhecido. O
pathname nativo permanece no backend e nunca é transportado como string Unicode
para a interface. Os cartões apresentam somente informações reais, sem
metadados fictícios ou rótulos fixos de reprodução.

A prévia é consultada sob demanda para os cartões visíveis. O Core lê a revisão
persistida e fornece a composição da primeira lâmina, sem criar uma sessão
editável, salvar o documento ou alterar a ordem dos recentes. A interface
reutiliza o desenho de lâmina compartilhado, incluindo páginas ativas, fundos,
sobreposições, fotos e suas transformações; não recompõe as regras do álbum.

As fotos e os decorativos usam as representações reduzidas já disponíveis no
Cache, por referências autorizadas. Essa consulta não inicia processamento dos
originais nem grava uma imagem de capa. O baseline do ADR 0005 continua válido:
não há preview persistido de lâmina nem novo formato de armazenamento de capa.

A miniatura repousa sobre uma superfície clara e quente (--ui-surface-muted),
levemente distinta do fundo da lista. Uma linha fina de baixo contraste e a
sombra curta das miniaturas definem o limite da lâmina, inclusive quando ela é
branca ou tem uma cor próxima do fundo. Esse acabamento envolve o papel inteiro,
sem aumentar sua área, recortar a composição ou criar uma moldura espessa.

Durante a consulta ou se o Projeto estiver indisponível, aparece uma pequena
representação neutra de **álbum fechado**, com capa, lombada discreta e borda de
páginas. Ela ocupa o centro da mesma área reservada, sem texto de carregamento,
aviso ou animação. O álbum fechado distingue a ausência de prévia de uma lâmina
realmente branca e evita deixar o cartão visualmente vazio.

Cartões com e sem prévia compartilham as mesmas dimensões e a ação de abrir.
O hover e o foco pertencem ao cartão inteiro. Uma mídia sem prévia em uma
composição disponível segue a apresentação degradada do desenho compartilhado.
A ausência da miniatura não impede a tentativa normal de abertura. Alterações
ainda não salvas em uma Janela de Projeto não aparecem na miniatura dos recentes.

A faixa inferior usa **uma única linha**: Nome do Projeto à esquerda e,
quando conhecida, a data da última abertura à direita, menor e discreta.
A data aparece como **Hoje**, **Ontem** ou **18/09/2026**. O tooltip compartilhado
e a descrição acessível informam também o horário local, por exemplo
**Última abertura: Hoje às 14:30**. O nome ocupa o espaço restante e usa reticências
quando necessário; nome, data e seta não se sobrepõem. A data conserva seu
espaço mesmo quando o nome é muito comprido ou não contém espaços.

O hover do horário pertence apenas à data: sair dela para o nome, a miniatura ou
outra parte do mesmo cartão fecha esse tooltip. Passar sobre um nome truncado
mostra o nome completo no mesmo padrão visual, com quebra de palavras longas;
nomes que já cabem não recebem uma dica redundante. Os tooltips não aumentam o
cartão e devem permanecer dentro da área visível. A navegação por teclado usa
o único botão de abrir o cartão, preserva acesso ao nome completo e ao horário
conhecido e permite fechar a dica com Escape; não há um foco adicional na data.

A linha “Projeto MyAlbuns” e a expressão genérica “Aberto recentemente” não
aparecem. Os cartões têm 160 px de altura, com 126 px reservados à miniatura
e uma faixa inferior de 32 px; o espaço da seta permanece reservado.
Sem data conhecida, a mesma linha mostra apenas o nome, sem uma linha vazia
abaixo nem texto substituto. Essa decisão do autor, de 22 de setembro de 2026,
substitui a apresentação inicial do rodapé em duas linhas.

A lista usa a abertura mais recente como ordenação decrescente. O backend
registra o instante e promove a entrada somente depois que o Host independente
confirma Ready; cancelamento ou falha anterior não cria, reordena ou atualiza
a data. Consultar a lista não grava no armazenamento.

Registros anteriores, sem instante de abertura, continuam disponíveis e
mostram apenas o nome. A data passa a existir na próxima abertura concluída.
Não se usa a data de modificação do arquivo nem o horário da consulta para
simular esse histórico. O timestamp pertence ao estado local dos recentes,
sem alterar o documento criativo ou o Cache.

## Favoritos

A Tela de Boas-vindas apresenta **Favoritos** antes de **Projetos recentes**.
Cada Projeto aparece em somente um grupo; grupos vazios são omitidos. A ordem
dentro de cada grupo continua sendo a da última abertura, sem reordenar o
histórico quando a estrela é acionada. O mesmo cartão, miniatura e rodapé são
usados nos dois grupos. A estrela de 16 px ocupa um botão de 28 px no canto
superior direito: ligada, fica preenchida em grafite e sempre visível; desligada,
aparece no hover ou foco. O botão possui nome acessível, estado pressionado e
tooltip, sem acionar a abertura. O foco permanece na ação após a troca de grupo.

O estado local de Projetos recentes guarda a marcação por identidade de Projeto,
separado do arquivo criativo e do Cache. A consulta não grava. Registros legados
sem a marcação são tratados como não favoritos; a próxima mutação publica o
campo opcional no mesmo esquema, sem migração em leitura. Favoritos persistem e
ficam fora do limite de 20 entradas não favoritas. Ao desfavoritar, aplica-se
esse limite em ordem cronológica, o que pode retirar da lista uma entrada antiga
sem excluir seu arquivo. Reabrir a mesma identidade conserva a marcação; uma
nova identidade no mesmo caminho não a herda. Falhas de gravação conservam a
lista anterior e usam o aviso operacional existente.

Os atalhos Windows `Ctrl+N` e `Ctrl+O` aparecem junto às ações e acionam,
respectivamente, `Novo Projeto` e `Abrir Projeto`; não são legendas decorativas.

## Transição de abertura

Depois que `Abrir Projeto` ou um cartão de `Projetos recentes` confirma um Projeto existente, a Tela de Boas-vindas sai da área visível antes que a janela de progresso de abertura apareça. Essa é a única operação que substitui visualmente a superfície solicitante durante o processamento. Um resultado `Ready` transfere o trabalho à Janela do Projeto e permite encerrar o processo global; uma falha restaura a Tela de Boas-vindas visível e apresenta o aviso em uma janela pertencente, à frente dela.

Enquanto a Tela de Boas-vindas estiver oculta, o progresso de abertura possui sua
própria entrada na barra de tarefas do Windows. Assim, o usuário pode voltar ao
acompanhamento depois de acessar outro programa. A entrada acompanha a mesma
janela durante a preparação das imagens, Recuperação e decisão de Cópia externa,
e é removida ao encerrar o diálogo. A abertura direta pelo Windows segue a mesma
regra, mesmo quando a Tela de Boas-vindas não chegou a aparecer.

Se o Host correlacionado detectar Recuperação ou uma Cópia externa somente leitura, a janela externa de progresso permanece a proprietária causal da tentativa e troca apenas seu conteúdo para a decisão aplicável. A Global não renderiza essa decisão dentro da própria WebView e a Janela do Projeto ainda não é exibida. Em Cópia externa, `Salvar cópia como…` abre o seletor nativo a partir dessa janela e continua o mesmo Host; cancelar somente o seletor retorna à decisão, enquanto `Cancelar` encerra a tentativa. Ativações posteriores aguardam o terminal dessa tentativa em vez de substituir seu owner.

Após a decisão de Recuperação e antes de apresentar o editor, o Host aplica as dimensões já orientadas do Cache verificado ao catálogo efetivo da sessão. A correspondência exige a mesma mídia e o mesmo caminho, inclusive para Fotos importadas ou religadas após o último salvamento. Isso preserva as proporções das miniaturas e dos Frames mesmo quando o Original está ausente, sem salvar o Projeto nem incorporar dados de Cache ao estado criativo recuperável.

`Novo Projeto` não herda essa exceção. O fluxo de criação ocupa a própria janela e qualquer seletor, confirmação, aviso ou progresso solicitado por ele preserva essa janela visível e bloqueada ao fundo, conforme o contrato de [diálogos pertencentes](0001-estrutura-da-janela-do-projeto.md#diálogos-pertencentes-a-uma-janela).

## Relação com as Janelas de Projeto

Cada Projeto permanece em uma Janela e Sessão do Projeto isoladas, hospedadas por
um processo próprio no papel interno de Host. O Processador de Imagens também
fica separado do host interativo e dos demais Projetos. O processo global é um
ponto de entrada descartável: depois de um terminal `Ready` válido, ele pode
encerrar sem afetar o Host ou possuir estado criativo. Uma nova entrada global
pode ser iniciada quando outra ação de abertura ou criação precisar dela; não há
coordenador global de Sessões.

Abrir diretamente um arquivo pelo Windows pode iniciar sua Janela de Projeto sem mostrar antes a Tela de Boas-vindas.

## Operações em lote

`Exportação em lote` pertence à Tela de Boas-vindas porque lê Projetos
persistidos encontrados em uma pasta e não precisa de um Projeto modelo aberto.
Na nova UI, a ação permanece desabilitada e explicitamente marcada como
placeholder. Quando sua porta de aplicação for implementada, ela abrirá a janela
dedicada de [Configuração da Exportação em lote](0006-configuracao-da-exportacao-em-lote.md).

`Geração de Projetos em lote` não aparece nessa tela. Ela é iniciada exclusivamente em uma Janela de Projeto, pois copia o estado visível daquele Projeto como modelo, inclusive alterações ainda não salvas, sem salvar ou modificar o original.
