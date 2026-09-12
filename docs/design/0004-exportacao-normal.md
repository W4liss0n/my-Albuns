---
status: accepted
document: design
updated: 2026-09-12
---

# Exportação normal

## Objetivo

A tela de Exportação reúne somente as decisões necessárias para gerar a saída final do Projeto aberto. Dimensões e DPI são herdados do Projeto e não podem ser substituídos nesse fluxo.

## Estrutura do diálogo

A Exportação normal usa um único diálogo modal com largura inicial de 800 px,
sem uma linha interna de título `Exportar`. A proposta abaixo foi aprovada
pelo usuário como refinamento da referência visual do projeto:

![Proposta aprovada para a Exportação normal](assets/0004-exportacao-aprovada.png)

Os blocos têm títulos acima dos controles, margens laterais amplas e espaço
entre eles, na seguinte ordem:

1. `Destino da exportação`: campo de pasta e botão `Escolher…` na mesma linha;
2. `Formato de exportação`: seletor `JPEG`, `PNG` ou `PDF`, acompanhado do
   slider e percentual de qualidade quando o formato é JPEG;
3. `Seleção de lâminas`: opções exclusivas `Todas as lâminas` e
   `Intervalo personalizado`, uma abaixo da outra. O intervalo tem um único
   campo ao lado. Na mesma linha, à direita, fica a opção independente
   `Exportar como páginas simples`.

`Todas as lâminas` é o padrão. O campo de intervalo permanece desabilitado
nessa seleção e preserva o texto ao alternar. O intervalo aceita uma lâmina
(`3`) ou uma faixa contínua (`3-8`). Páginas simples desmarcado corresponde
à saída por lâmina; marcado corresponde à saída por página. Em janelas mais
estreitas, a opção de páginas simples passa para a linha seguinte.

O rodapé fixo apresenta a quantidade calculada de arquivos para JPEG/PNG ou de páginas para PDF, além de `Cancelar` e `Exportar`.

Na abertura, a pasta de destino padrão é obtida antes de apresentar o diálogo.
A janela aparece diretamente com os campos prontos para edição, sem um estado
transitório de preparação que altere sua altura. Se a consulta falhar, o diálogo
já abre editável com a mensagem de erro e permite escolher outra pasta.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Destino da exportação                                          │
│  [ caminho calculado ou escolhido ]                [ Escolher ] │
│                                                                 │
│  Formato de exportação                                          │
│  [ JPEG ▾ ]    Qualidade: ─────────● 100%                         │
│                                                                 │
│  Seleção de lâminas                                             │
│  ● Todas as lâminas                                             │
│  ○ Intervalo personalizado [ 3-8 ]    □ Páginas simples          │
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

`Exportar Lâmina`, acionado pelo menu de contexto, abre o mesmo diálogo com
`Intervalo personalizado` selecionado e o número da Lâmina de origem preenchido no
campo. Todas as demais opções continuam editáveis antes de iniciar.

Ao selecionar `Intervalo personalizado`, o campo vazio permanece neutro. Um valor preenchido inválido recebe indicação no campo e orientação em tooltip, sem adicionar linhas ou alterar a altura da janela. O botão `Exportar` permanece desabilitado enquanto o intervalo estiver vazio ou inválido. A operação mantém os arquivos fora do intervalo. Uma Exportação integral é a única operação que restabelece um conjunto completo autoritativo no destino.

## Pré-validação

Ao acionar `Exportar`, placeholders e originais necessários ausentes ou indisponíveis não são exibidos dentro deste formulário. O programa abre a [Tela de Problemas](0005-tela-de-problemas.md) filtrada para a Exportação, que identifica cada bloqueio e oferece as ações apropriadas antes de qualquer progresso.

## Preparação e Publicação

JPEG e PNG compartilham o namespace `{nome-do-projeto}_{índice decimal com largura mínima de três dígitos}` nos modos `Por lâmina` e `Por página`; `001` a `999` conservam três dígitos e índices maiores crescem normalmente. O nome isolado não identifica o modo usado. O mapeamento de qualidade, os formatos e o comportamento acima de `999` pertencem ao [Contrato do Renderizador final](0019-contrato-do-renderizador-final.md).

Ao iniciar, a operação adquire o `OperationLease` exclusivo; não existe fila de espera. O lease reserva em conjunto a concessão global, a pausa do Cache e o Processador de Imagens, e garante a devolução dos três recursos em sucesso, falha, cancelamento ou queda — a Exportação não os orquestra individualmente. O contrato do lease está em [Propriedade de estado e módulos do núcleo](0012-propriedade-de-estado-e-modulos-do-nucleo.md). Cancelamento e progresso continuam pertencendo somente à tentativa.

O `ExportPipeline` possui internamente planejamento, execução e `Publisher`. Primeiro recebe `RenderSnapshot` e opções e devolve o plano com todas as dependências e raízes necessárias. O proprietário captura então o `RootBindingPlan` definido pela [política de caminhos](0011-resolucao-e-politica-de-caminhos.md) e inicia a execução. Se host e Processador participarem da tentativa, ambos recebem o mesmo plano; todo Original necessário é aberto uma vez no conjunto imutável da tentativa e reutilizado por todas as Unidades de Exportação.

Todas as saídas selecionadas são renderizadas e verificadas em uma pasta de preparação reservada dentro do próprio Destino antes da Publicação. Isso mantém preparação e nomes finais na mesma árvore de destino. Uma falha nessa fase não modifica os nomes finais.

Depois da preparação integral, o `Publisher` promove cada arquivo separadamente ao nome final com atomicidade por arquivo quando o Destino suportar. Não há rollback do conjunto: uma falha durante a Publicação pode deixar mistura entre saídas anteriores e novas, deve informar essa condição e não remove Saídas órfãs.

Saídas órfãs só são removidas depois da Publicação bem-sucedida de uma Exportação integral confirmada para sobrescrita. Exportações parciais nunca removem arquivos fora do intervalo.
