---
status: accepted
document: design
---

# Exportação normal

## Objetivo

A tela de Exportação reúne somente as decisões necessárias para gerar a saída final do Projeto aberto. Dimensões e DPI são herdados do Projeto e não podem ser substituídos nesse fluxo.

## Estrutura do diálogo

A Exportação normal usa um único diálogo modal com quatro grupos:

1. `Escopo`: `Álbum inteiro` ou `Intervalo de Lâminas`;
2. `Modo`: `Por lâmina` ou `Por página`;
3. `Formato`: `JPEG`, `PNG` ou `PDF`;
4. `Destino`: pasta padrão calculada ou outra pasta local, UNC, mapeada ou longa escolhida pelo usuário.

O rodapé fixo apresenta a quantidade calculada de arquivos para JPEG/PNG ou de páginas para PDF, além de `Cancelar` e `Exportar`.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Exportar                                                        │
├──────────────────────────────────────────────────────────────────┤
│  Escopo       Álbum inteiro  |  Intervalo [ início ] [ fim ]     │
│  Modo         Por lâmina | Por página                            │
│  Formato      JPEG | PNG | PDF                                   │
│  Qualidade    ─────────●──────        somente para JPEG          │
│  Destino      [ caminho calculado ou escolhido ] [ Escolher... ] │
├──────────────────────────────────────────────────────────────────┤
│  28 arquivos                              Cancelar   Exportar     │
└──────────────────────────────────────────────────────────────────┘
```

## Qualidade

O slider de qualidade aparece somente quando `JPEG` está selecionado na Exportação normal. Trocar para PNG ou PDF remove o controle em vez de deixá-lo desabilitado.

Cada abertura do diálogo inicia o slider em qualidade máxima. Dois cliques no controle restauram imediatamente esse mesmo valor.

Qualidade é uma opção daquela operação: não modifica o Projeto, não marca alterações pendentes, não participa de Undo/Redo e não é carregada como preferência da próxima Exportação. A Exportação em lote não apresenta esse slider.

Quando o formato do lote é JPEG, a codificação usa obrigatoriamente qualidade máxima.

## Entrada contextual

`Exportar Lâmina`, acionado pelo menu de contexto, abre o mesmo diálogo com `Intervalo de Lâminas` selecionado e a Lâmina de origem preenchida como início e fim. Todas as demais opções continuam editáveis antes de iniciar.

Quando `Intervalo de Lâminas` está selecionado, o diálogo informa que a operação não remove arquivos fora do intervalo e não consegue inferir o modo das saídas já existentes pelo nome. Uma Exportação integral é a única operação que restabelece um conjunto completo autoritativo no destino.

## Pré-validação

Ao acionar `Exportar`, placeholders e originais necessários ausentes ou indisponíveis não são exibidos dentro deste formulário. O programa abre a [Tela de Problemas](0005-tela-de-problemas.md) filtrada para a Exportação, que identifica cada bloqueio e oferece as ações apropriadas antes de qualquer progresso.

## Preparação e Publicação

JPEG e PNG compartilham o namespace `{nome-do-projeto}_{NNN}` nos modos `Por lâmina` e `Por página`; o nome isolado não identifica o modo usado.

Ao iniciar, a operação adquire o `OperationLease` exclusivo; não existe fila de espera. O lease reserva em conjunto a concessão global, a pausa do Cache e o Processador de Imagens, e garante a devolução dos três recursos em sucesso, falha, cancelamento ou queda — a Exportação não os orquestra individualmente. O contrato do lease está em [Propriedade de estado e módulos do núcleo](0012-propriedade-de-estado-e-modulos-do-nucleo.md). Cancelamento e progresso continuam pertencendo somente à tentativa.

O `ExportPipeline` possui internamente planejamento, execução e `Publisher`. Primeiro recebe `RenderSnapshot` e opções e devolve o plano com todas as dependências e raízes necessárias. O proprietário captura então o `RootBindingPlan` definido pela [política de caminhos](0011-resolucao-e-politica-de-caminhos.md) e inicia a execução. Se host e Processador participarem da tentativa, ambos recebem o mesmo plano; todo original necessário ainda é aberto e verificado.

Todas as saídas selecionadas são renderizadas e verificadas em uma pasta de preparação reservada dentro do próprio Destino antes da Publicação. Isso mantém preparação e nomes finais na mesma árvore de destino. Uma falha nessa fase não modifica os nomes finais.

Depois da preparação integral, o `Publisher` promove cada arquivo separadamente ao nome final com atomicidade por arquivo quando o Destino suportar. Não há rollback do conjunto: uma falha durante a Publicação pode deixar mistura entre saídas anteriores e novas, deve informar essa condição e não remove Saídas órfãs.

Saídas órfãs só são removidas depois da Publicação bem-sucedida de uma Exportação integral confirmada para sobrescrita. Exportações parciais nunca removem arquivos fora do intervalo.
