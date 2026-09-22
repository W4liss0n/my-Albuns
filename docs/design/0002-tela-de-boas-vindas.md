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
capa visual, o Nome do Projeto, metadados secundários e a indicação de abertura.
Clicar em qualquer ponto do cartão abre o Projeto correspondente.

O cartão mostra a **primeira lâmina salva do Projeto**, no lugar da representação
genérica da referência original. Essa decisão do autor, de 22 de setembro de
2026, refina somente o conteúdo da miniatura; a grade, as ações e a hierarquia
da tela permanecem. A lâmina mantém sua proporção e cabe inteira na área
reservada, sem cortar a composição para preencher o cartão.

O frontend recebe o Nome do Projeto e uma Identidade opaca para solicitar sua
reabertura e sua prévia. O pathname nativo permanece no backend e nunca é
transportado como string Unicode para a interface. Fixação e metadados ainda
sem contrato continuam marcados como placeholders de reprodução.

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

A altura dos cartões, o alinhamento dos nomes e a ação de abrir permanecem.
O hover e o foco pertencem ao cartão inteiro. Uma mídia sem prévia em uma
composição disponível segue a apresentação degradada do desenho compartilhado.
A ausência da miniatura não impede a tentativa normal de abertura. Alterações
ainda não salvas em uma Janela de Projeto não aparecem na miniatura dos recentes.

A lista usa a abertura mais recente como ordenação decrescente. A entrada passa
para o topo somente depois que o Host independente confirma `Ready`; cancelamento
ou falha anterior não cria nem reordena o item.

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
