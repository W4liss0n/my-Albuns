---
status: accepted
document: design
---

# Progresso de operações

## Objetivo

O aplicativo reutiliza uma única representação simples para operações que precisam mostrar progresso. O componente não expõe a organização interna em processos, threads, filas ou trabalhos simultâneos.

Cada tentativa fornece seu próprio `ProgressSink` ao componente. A janela somente apresenta os eventos recebidos: não possui o trabalho, a exclusividade global ou o token de cancelamento e não mantém um serviço global de progresso.

A representação também não decide o ciclo de vida da janela solicitante. Ela segue a política do fluxo que a abriu: o padrão de [diálogo pertencente](0001-estrutura-da-janela-do-projeto.md#diálogos-pertencentes-a-uma-janela) preserva a proprietária visível e bloqueada, e somente a [transição de abertura de um Projeto existente](0002-tela-de-boas-vindas.md#transição-de-abertura) a substitui temporariamente.

## Progresso determinado

Quando a operação conhece um total confiável, a janela mostra:

- uma linha curta com a etapa ou unidade atual;
- uma única barra de progresso geral;
- a porcentagem concluída;
- uma estimativa de tempo somente quando a operação oferece esse dado confiável.

```text
┌──────────────────────────────────────────────┐
│  Exportando                                  │
│                                              │
│  ███████████████░░░░░░░░░░░░                │
│  43%                         cerca de 2 min   │
└──────────────────────────────────────────────┘
```

A linha de estado pode usar `X/Y` para a unidade própria da operação, como
`Lâmina 18 de 42`. A janela não inventa tempo restante quando o produtor de
progresso não consegue estimá-lo.

## Progresso em lote

O lote reutiliza a mesma barra geral e acrescenta somente um resumo compacto do
conjunto: item atual, estado desse item, posição `X/Y` e uma síntese da fila. Não
exibe uma tabela nem os trabalhos simultâneos.

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
- Itens ignorados ou com falha fecham o progresso e abrem a [Tela de Problemas](0005-tela-de-problemas.md) no contexto de resultado.
