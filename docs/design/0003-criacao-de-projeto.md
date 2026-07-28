---
status: accepted
document: design
---

# Criação de Projeto

## Objetivo

O fluxo de `Novo Projeto` coleta apenas as definições iniciais do Álbum. Nome e Localização permanecem sob responsabilidade do diálogo nativo do Windows.

## Estrutura

A criação possui exatamente duas etapas dentro do aplicativo:

1. `Dimensões`, dedicada à base física e estrutural do Álbum;
2. `Personalização`, dedicada aos padrões visuais iniciais.

### Dimensões

A primeira etapa é um formulário simples dividido em três grupos.

`Documento` contém:

- Unidade de medida;
- largura e altura da Lâmina;
- DPI.

`Estrutura` contém:

- quantidade inicial de Lâminas;
- formato da primeira Lâmina;
- formato da última Lâmina.

`Áreas técnicas` contém:

- Sangria;
- Área de segurança.

Largura e altura representam a Lâmina inteira. A largura de cada Página é calculada como sua metade.

A etapa não possui reprodução gráfica da Lâmina. Um resumo somente de leitura apresenta continuamente a dimensão da Lâmina, a dimensão calculada de cada Página e o DPI, por exemplo:

`Lâmina 60 × 24 cm · Páginas 30 × 24 cm · 300 DPI`

```text
┌──────────────────────────────────────────────────────────────────┐
│  Dimensões                                                       │
├──────────────────────────────────────────────────────────────────┤
│  Documento                                                       │
│  Unidade · Largura · Altura · DPI                                │
│                                                                  │
│  Estrutura                                                       │
│  Quantidade · Primeira Lâmina · Última Lâmina                    │
│                                                                  │
│  Áreas técnicas                                                  │
│  Sangria · Área de segurança                                     │
│                                                                  │
│  Lâmina 60 × 24 cm · Páginas 30 × 24 cm · 300 DPI               │
├──────────────────────────────────────────────────────────────────┤
│  Cancelar                                             Próximo    │
└──────────────────────────────────────────────────────────────────┘
```

### Validação

`Próximo` valida todos os campos da etapa `Dimensões` e só avança quando o conjunto estiver válido. Cada problema aparece em texto junto ao campo correspondente, e o foco é transferido para o primeiro campo inválido.

O fluxo não usa um modal genérico para listar erros de preenchimento. Corrigir um campo atualiza seu estado e o resumo calculado imediatamente.

### Personalização

A segunda etapa contém:

- Background padrão;
- Overlay padrão;
- Padrão dos Frames.

Background e Overlay oferecem `Escolher imagem...`, que abre o seletor nativo do Windows. A imagem escolhida permanece provisória e já pode ser exibida na etapa, mas só é vinculada à aba `Decorativos` quando a criação do Projeto for concluída. Cancelar qualquer parte do fluxo não importa nem copia o arquivo.

### Prévia viva

A etapa `Personalização` contém uma reprodução de Lâmina com Frames de demonstração. Ela mostra em conjunto:

- o Background atual;
- o Overlay atual;
- a presença, a cor e a espessura da Borda padrão dos Frames.

A reprodução mostra sempre uma Lâmina dupla e mantém a proporção de largura e altura definida na etapa `Dimensões`. O formato escolhido para a primeira ou a última Lâmina não desativa lados nessa demonstração, pois sua finalidade é permitir a configuração conjunta dos escopos esquerdo, direito e de Ambos os lados.

Qualquer alteração nesses controles atualiza a reprodução imediatamente. As imagens provisórias escolhidas para Background ou Overlay também são compostas na prévia antes da criação do Projeto.

A reprodução reutiliza a seleção espacial de `Design do Álbum`:

- hover no lado esquerdo realça somente o lado esquerdo;
- hover no lado direito realça somente o lado direito;
- hover na região central realça os dois lados;
- clicar fixa o escopo selecionado.

Os controles de Background e Overlay atuam no escopo fixado. Mover o ponteiro sem clicar produz somente o realce temporário e não troca a configuração selecionada.

A reprodução é somente visual: sua Lâmina, seus Frames e qualquer conteúdo demonstrativo não são copiados para o Projeto criado.

### Disposição da etapa

`Personalização` usa duas colunas durante toda a etapa:

- a coluna esquerda, maior, mantém a reprodução da Lâmina;
- a coluna direita contém os controles de Background, Overlay e Padrão dos Frames;
- o rodapé fixo contém `Voltar`, `Cancelar` e `Criar`.

Os controles podem rolar dentro de sua região quando necessário, sem retirar a reprodução da área visível. O rodapé também permanece acessível independentemente da rolagem.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Personalização                                                  │
├───────────────────────────────────────────┬──────────────────────┤
│                                           │  Background          │
│                                           │  Overlay             │
│          reprodução da Lâmina             │  Padrão dos Frames   │
│                                           │                      │
│                                           │                      │
├───────────────────────────────────────────┴──────────────────────┤
│  Voltar                         Cancelar              Criar      │
└──────────────────────────────────────────────────────────────────┘
```

O fluxo permite voltar à etapa anterior sem perder valores. `Cancelar` encerra a criação sem produzir arquivo. `Criar` existe na etapa final e, depois de validar as duas etapas, abre o diálogo nativo do Windows.

## Nome e Localização

O Nome do Projeto e o destino não são campos das etapas do aplicativo. No diálogo nativo, o usuário escolhe a pasta e informa o nome do arquivo.

Cancelar esse diálogo retorna à etapa `Personalização` com todos os valores preservados. Nenhum arquivo, Identidade de Projeto ou entrada em `Projetos recentes` é criado antes da confirmação válida do diálogo nativo.

O destino pode ser local, UNC, unidade mapeada ou caminho verbatim local/UNC. A [política de caminhos](0011-resolucao-e-politica-de-caminhos.md) valida a forma totalmente qualificada e a capacidade de gravação antes de criar o arquivo; falha não deixa Projeto parcial.
