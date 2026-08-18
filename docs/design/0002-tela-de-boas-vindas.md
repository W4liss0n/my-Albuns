---
status: accepted
document: design
---

# Tela de Boas-vindas

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

A lista usa a abertura mais recente como ordenação decrescente. A entrada passa
para o topo somente depois que o Host independente confirma `Ready`; cancelamento
ou falha anterior não cria nem reordena o item.

Os atalhos Windows `Ctrl+N` e `Ctrl+O` aparecem junto às ações e acionam,
respectivamente, `Novo Projeto` e `Abrir Projeto`; não são legendas decorativas.

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

`Exportação em lote` pertence à Tela de Boas-vindas porque lê Projetos persistidos encontrados em uma pasta e não precisa de um Projeto modelo aberto. A ação abre a janela dedicada de [Configuração da Exportação em lote](0006-configuracao-da-exportacao-em-lote.md).

`Geração de Projetos em lote` não aparece nessa tela. Ela é iniciada exclusivamente em uma Janela de Projeto, pois copia o estado visível daquele Projeto como modelo, inclusive alterações ainda não salvas, sem salvar ou modificar o original.
