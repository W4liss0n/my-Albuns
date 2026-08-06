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
- `Exportação em lote`, para selecionar e processar uma pasta com Projetos persistidos;
- `Configurações`, para preferências do aplicativo e opções globais;
- `Ajuda`, para acesso ao suporte e às informações do programa.

## Hierarquia visual

`Projetos recentes` é a área dominante e utiliza a maior parte da janela. Ao lado dela, um grupo de ações principais dá acesso imediato a:

- `Novo Projeto`;
- `Abrir Projeto`;
- `Exportação em lote`.

`Configurações` e `Ajuda` são ações secundárias e ocupam uma região inferior discreta, sem competir visualmente com a criação, a abertura ou a lista de trabalhos. `Configurações` abre a janela global descrita em [Configurações do aplicativo](0009-configuracoes-do-aplicativo.md); se ela já estiver aberta, apenas a focaliza.

```text
┌──────────────────────────────────────────────────────────────────┐
│                            MyAlbuns                               │
├──────────────────────────────────────────┬───────────────────────┤
│                                          │  Novo Projeto         │
│           Projetos recentes              │  Abrir Projeto        │
│              área principal              │  Exportação em lote   │
│                                          │                       │
├──────────────────────────────────────────┴───────────────────────┤
│                                      Configurações · Ajuda       │
└──────────────────────────────────────────────────────────────────┘
```

O protótipo definirá proporções, espaçamentos e dimensões finais sem alterar essa hierarquia.

## Projetos recentes

Os Projetos recentes aparecem em uma lista textual. Cada item expõe ao frontend
somente o Nome do Projeto e uma Identidade opaca usada para solicitar sua
reabertura. O pathname nativo permanece no armazenamento do backend e nunca é
transportado como string Unicode para a interface.

Não há miniatura ou data no item da primeira versão. Clicar em qualquer ponto da linha abre o Projeto correspondente.

A lista usa a abertura mais recente como ordenação decrescente. A entrada passa
para o topo somente depois que o Host independente confirma `Ready`; cancelamento
ou falha anterior não cria nem reordena o item.

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
