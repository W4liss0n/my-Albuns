---
status: accepted
document: design
---

# Progresso de operações

## Objetivo

O aplicativo reutiliza uma única representação simples para operações que precisam mostrar progresso. O componente não expõe a organização interna em processos, threads, filas ou trabalhos simultâneos.

Cada tentativa fornece seu próprio `ProgressSink` ao componente. A janela somente apresenta os eventos recebidos: não possui o trabalho, a exclusividade global ou o token de cancelamento e não mantém um serviço global de progresso.

## Progresso determinado

Quando a operação conhece um total confiável, a janela mostra:

- uma única barra de progresso geral;
- uma contagem no formato `X/Y`.

```text
┌──────────────────────────────────────────────┐
│  Exportando                                  │
│                                              │
│  ███████████████░░░░░░░░░░░░                │
│  18/42                                       │
└──────────────────────────────────────────────┘
```

`X/Y` representa unidades concluídas sobre unidades totais conforme o tipo de operação.

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
- nomes de trabalhos executados em paralelo;
- contagens separadas de concluídos, ignorados ou com falha;
- detalhes técnicos de processos ou threads.

A Exportação em lote usa o modo determinado com seu progresso geral e processa um Projeto por vez no MVP. `X` avança somente quando o item alcança estado concluído, ignorado ou falho; o componente não expõe o Checkpoint do lote.

A Limpeza total do Cache pode reutilizar o mesmo componente quando executada sem Projeto ou Processador ativo, inclusive na inicialização agendada. Ela não pausa Janelas ativas nem remove Cache em uso no MVP.

## Conclusão

A janela de progresso nunca se converte em relatório.

- Sucesso integral fecha a janela e mostra uma confirmação curta.
- Itens ignorados ou com falha fecham o progresso e abrem a [Tela de Problemas](0005-tela-de-problemas.md) no contexto de resultado.
