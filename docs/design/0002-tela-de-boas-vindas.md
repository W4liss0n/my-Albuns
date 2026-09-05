---
status: accepted
document: design
---

# Tela de Boas-vindas

**Referência visual vigente:** [Tela de Boas-vindas](../references/ui-programa-diagramacao/Boas-vindas.dc.html)

## Objetivo

A Tela de Boas-vindas é a superfície principal visível do processo `MyAlbuns.exe`. Ela funciona como ponto de entrada e coordenação do aplicativo, sem incorporar Canvas, composição ou qualquer estado criativo de um Projeto.

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

O contrato real ainda expõe ao frontend somente o Nome do Projeto e uma
Identidade opaca usada para solicitar sua reabertura. O pathname nativo permanece
no armazenamento do backend e nunca é transportado como string Unicode para a
interface. Enquanto capa, fixação e metadados não existirem nesse contrato, esses
trechos do cartão são placeholders de reprodução e permanecem marcados no código
com `PLACEHOLDER UI` e `data-placeholder-feature`.

Uma capa real não possui contrato nesta versão. Conforme o ADR 0005, previews
persistidos de Lâmina só podem ser introduzidos se medições demonstrarem que o
baseline não atende. Até que essa evidência exista e a decisão arquitetural seja
reavaliada, este desenho não define geração, persistência, transporte, esquema
nem momento de atualização para uma capa; o cartão conserva exclusivamente o
placeholder descrito acima.

A lista usa a abertura mais recente como ordenação decrescente. A entrada passa
para o topo somente depois que o Host independente confirma `Ready`; cancelamento
ou falha anterior não cria nem reordena o item.

Os atalhos Windows `Ctrl+N` e `Ctrl+O` aparecem junto às ações e acionam,
respectivamente, `Novo Projeto` e `Abrir Projeto`; não são legendas decorativas.

## Transição de abertura

Depois que `Abrir Projeto` ou um cartão de `Projetos recentes` confirma um Projeto existente, a Tela de Boas-vindas sai da área visível antes que a janela de progresso de abertura apareça. Essa é a única operação que substitui visualmente a superfície solicitante durante o processamento. Um resultado `Ready` transfere o trabalho à Janela do Projeto e permite encerrar o processo global; uma falha restaura a Tela de Boas-vindas visível e apresenta o aviso em uma janela pertencente, à frente dela.

Se o Host correlacionado detectar Recuperação ou uma Cópia externa somente leitura, a janela externa de progresso permanece a proprietária causal da tentativa e troca apenas seu conteúdo para a decisão aplicável. A Global não renderiza essa decisão dentro da própria WebView e a Janela do Projeto ainda não é exibida. Em Cópia externa, `Salvar cópia como…` abre o seletor nativo a partir dessa janela e continua o mesmo Host; cancelar somente o seletor retorna à decisão, enquanto `Cancelar` encerra a tentativa. Ativações posteriores aguardam o terminal dessa tentativa em vez de substituir seu owner.

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
