---
status: accepted
document: design
---

# Tela de Problemas

## Objetivo

A Tela de Problemas é uma superfície tabular reutilizável para diferentes categorias de problema do aplicativo. Ela não pertence exclusivamente à Exportação: cada fluxo pode abri-la com as linhas e ações pertinentes ao seu contexto.

Na primeira versão, ela não possui item permanente em `Exibir` nem outro comando para abertura manual. A tela surge automaticamente quando um fluxo encontra problemas e já começa filtrada para aquela operação.

Ela não bloqueia o acesso necessário às Janelas de Projeto. A operação que a originou permanece pendente enquanto o usuário abre e corrige Projetos, sem iniciar processamento final em segundo plano.

## Tabela

As colunas se adaptam ao objeto tratado pelo fluxo. Quando o problema pertence a um Projeto ou a uma operação em lote, a estrutura padrão contém:

- `Projeto`;
- `Problema`;
- `Ações`.

O wireframe abaixo exemplifica a variante de Exportação em lote; ações como `Ignorar` e `Relinkar todos` não aparecem em todos os contextos.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Problemas                                                      │
├──────────────────┬──────────────────────────┬────────────────────┤
│ Projeto          │ Problema                 │ Ações              │
├──────────────────┼──────────────────────────┼────────────────────┤
│ Álbum 001        │ 2 Frames vazios          │ Abrir | Ignorar    │
│ Álbum 002        │ 4 imagens ausentes       │ Relinkar | Ignorar │
│ Álbum 003        │ 1 imagem ausente         │ Relinkar | Ignorar │
├──────────────────┴──────────────────────────┴────────────────────┤
│ Relinkar todos          Cancelar      Continuar Exportação      │
└──────────────────────────────────────────────────────────────────┘
```

Outros contextos podem substituir ou acrescentar colunas quando o objeto afetado não for um Projeto. `Ações` também pode ser omitida quando não houver correção possível dentro daquele resultado.

## Importação no Painel

Uma importação com vários arquivos possui sucesso parcial. Arquivos válidos são acrescentados ao Painel mesmo quando outros forem inválidos, corrompidos ou incompatíveis; uma falha não reverte os itens já aceitos. Caminhos já presentes na aba seguem a regra normal de duplicata e não aparecem como falha.

Se houver alguma rejeição, a Tela de Problemas é aberta ao término com:

- `Arquivo`;
- `Motivo`.

Essa visão é um resultado da operação já concluída. Fechá-la não desfaz os itens importados com sucesso.

## Visão de Exportação

Na Exportação, `Abrir Projeto` é a ação de um problema com Frame placeholder. `Relinkar` é a ação de um Arquivo original cuja ausência foi confirmada sob uma origem acessível. Arquivo indisponível preserva seu vínculo e oferece `Tentar novamente`; essa ação cria nova tentativa de acesso sem Undo/Redo, Religação ou Salvamento.

Depois de qualquer correção, a validação é executada novamente e a tabela remove ou atualiza os problemas resolvidos.

Na visão de Exportação, `Continuar Exportação` permanece desabilitado enquanto houver qualquer pendência sem decisão. Ao corrigir a última linha ou, no lote, ignorar explicitamente seu Projeto, o botão é habilitado, mas o processamento não começa automaticamente: o usuário precisa confirmá-lo.

Fechar ou cancelar a Tela de Problemas encerra a tentativa de Exportação pendente. Relinks realizados na sessão de um Projeto individual permanecem como alterações não salvas. O mapa temporário produzido para um lote é descartado. Esse mapa funcional não é o contexto técnico da [política de caminhos](0011-resolucao-e-politica-de-caminhos.md), que reutiliza fatos de raízes somente dentro de cada tentativa de processamento.

## Exportação individual

Na Exportação normal, `Relinkar` abre um seletor de pasta para o usuário informar onde estão as Fotos daquele Projeto. A busca percorre essa pasta recursivamente e exige o nome e a extensão exatos do Arquivo ausente.

Somente uma correspondência única é aceita. Nenhuma ou várias mantêm a pendência; uma correspondência aceita atualiza a referência na sessão aberta. A mudança:

- participa de Undo/Redo;
- marca o Projeto como alterado;
- não executa Salvamento automático;
- pode ser usada imediatamente pela Exportação normal, que trabalha com o estado visível.

## Exportação em lote

No lote, `Relinkar` em uma linha solicita a pasta das Fotos daquele Projeto e faz a mesma busca recursiva, exata e não ambígua. A correspondência integra somente o mapa temporário do lote e não atualiza o arquivo persistido.

O contexto do lote também acrescenta a ação global `Relinkar todos`. Ela solicita uma pasta raiz onde se encontram as pastas de Fotos dos Projetos.

Cada Projeto problemático também oferece `Ignorar neste lote`. A escolha retira explicitamente aquele Projeto da execução e resolve de uma vez todas as suas linhas na tabela, sem alterar seu arquivo. O Projeto e os motivos permanecem registrados como ignorados no resumo final.

Para cada Projeto, o programa procura recursivamente, sob a raiz escolhida, uma pasta cujo nome seja exatamente igual ao Nome do Projeto. Dentro dela, procura também recursivamente o nome e a extensão exatos de cada Arquivo ausente.

Somente uma correspondência única é aceita automaticamente. Se não houver resultado ou houver mais de um, o item permanece na tabela para evitar uma ligação ambígua. Uma Foto encontrada para um álbum nunca é usada para resolver automaticamente outro álbum.

Essas associações formam um mapa temporário da execução. Elas:

- não regravam arquivos de Projeto;
- não participam de Undo/Redo;
- não interferem em Projetos abertos ou em mudanças não salvas;
- são descartadas quando o lote termina ou é cancelado.

A tabela pertence à preparação do lote. O Modo de lote exclusivo e o progresso começam somente depois dessa etapa, permitindo que `Abrir Projeto` funcione enquanto os problemas estão sendo avaliados. Correções criativas feitas no editor, como preencher placeholders, precisam ser salvas antes de uma nova verificação, porque o lote reabre a versão persistida.

Para o lote, `Continuar Exportação` é habilitado quando cada problema tiver sido corrigido ou seu Projeto tiver sido explicitamente ignorado. A Exportação normal também exige `Continuar Exportação` depois de resolver suas pendências, mas não oferece `Ignorar neste lote`.

## Geração de Projetos em lote

Quando um caminho planejado já contém um Projeto, a Tela de Problemas apresenta uma linha de conflito para esse destino.

Cada linha oferece:

- `Sobrescrever`, para substituir somente aquele Projeto;
- `Ignorar`, para manter o Projeto existente e retirar aquele item da geração.

O contexto também oferece `Sobrescrever todos` e `Ignorar todos`. As ações globais aplicam a mesma decisão a todos os itens compatíveis, sem esconder os casos que não puderem recebê-la.

Se o Projeto de destino estiver aberto, `Sobrescrever` fica indisponível para aquela linha. `Sobrescrever todos` também não o inclui. Enquanto ele permanecer aberto, a única decisão possível para esse item é `Ignorar`; para substituí-lo, o usuário precisa fechá-lo e executar uma nova verificação.

A geração permanece pendente até que cada conflito tenha uma decisão. Mesmo depois da última decisão, nenhum arquivo é criado ou sobrescrito automaticamente: o usuário ainda confirma explicitamente `Continuar Geração`.

## Resultado de operações

A mesma Tela de Problemas apresenta resultados que exigem atenção após uma operação. Nesse contexto, a tabela identifica no mínimo `Projeto`, `Resultado` e as ações disponíveis, incluindo o motivo de itens ignorados ou com falha.

Ela só é aberta quando há algo a tratar. Sucesso integral usa uma confirmação curta e não abre uma tabela vazia.
