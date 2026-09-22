---
status: accepted
document: design
date: 2026-09-22
implementation-readiness: ready-for-agent
---

# Atualização das Configurações

A revisão de controles de 22/09/2026 identificou o mesmo protocolo de consulta
e ação nas seções de Prévias temporárias e Photoshop. O usuário aprovou a
centralização desse comportamento, preservando as decisões do design 0009.

## Proprietário compartilhado

`src/settings/useSettingsStatus.ts` concentra a consulta ao abrir e recuperar
foco, a precedência da requisição mais recente, o descarte de respostas antigas,
o bloqueio de outra ação durante uma atualização e o encerramento das consultas
quando a seção é desmontada. Cada seção possui uma instância independente: uma
limpeza pendente não impede escolher uma instalação na outra aba.

A ação explícita invalida consultas anteriores. Recuperar o foco durante essa
ação não inicia outra leitura. Sucesso e falha antigos não substituem a escolha
recente nem publicam mensagens após o encerramento. Uma falha vigente libera os
controles para nova tentativa. A interpretação do resultado só ocorre enquanto
a requisição ainda pertence à instância ativa.

## Responsabilidades preservadas

`CacheSettings` conserva a confirmação, a chamada de limpeza seguida da consulta
do estado atualizado e as mensagens de espaço liberado ou agendamento.
`PhotoshopSettings` conserva a seleção, a localização, a tradução de seus erros
e o resultado nulo do seletor cancelado, que mantém a instalação anterior.

Não há mudança de visual, textos, persistência ou contrato com Rust. As seções
continuam montadas ao alternar abas. As regras de limpeza e de instalações
permanecem nos proprietários existentes; não foram transferidas para a UI.
`usePhotoshop`, `useWorkspacePreferences` e as demais operações do programa
conservam seus protocolos próprios.

## Verificação

Os testes atravessam `SettingsWindow` e `PhotoshopSettings`, cobrindo leitura
inicial e ao foco, respostas fora de ordem, leitura antiga depois de uma ação,
sucesso e erro atrasados, ação pendente, cancelamento, nova tentativa após
falha, desmontagem e independência entre abas. Não dependem da estrutura
interna do hook compartilhado.

A apresentação é conferida nos nove cenários de Configurações já declarados
no manifesto de aceitação visual: instalações detectadas, ausentes, inválidas
e janela pequena; prévias em tamanho normal e reduzido; limpeza agendada;
confirmação normal e reduzida.

O ciclo de efeitos segue a limpeza e o descarte de resultados descritos na
[documentação do React](https://react.dev/learn/synchronizing-with-effects#fetching-data).
A implementação usa React 19.2.8; a consulta versionada disponível foi a 19.2.7,
complementada pela documentação oficial do ciclo de vida dos efeitos.
