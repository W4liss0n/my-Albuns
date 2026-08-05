---
status: historical
document: technical-research
ticket: fase-2-fluxo-persistente-08
date: 2026-08-03
updated: 2026-08-05
---

# Fechar o contrato público do ProjectStore

Type: grilling

Status: resolved

Blocked by: 01 — Reconciliar a fronteira de caminhos da próxima fase; 04 — Provar o Salvamento atômico preservando a trava do Projeto; 06 — Decidir o contrato do arquivo de Projeto v1

## Question

Quais operações e resultados tipados formam o contrato mínimo de criação, abertura e Salvamento do `ProjectStore` para esta fase, incluindo revisão esperada, confirmação da revisão salva, conflito, cancelamento e falha?

A decisão deve manter `ProjectStore` como proprietário de formato, migração e substituição atômica, `ProjectSession` como único proprietário mutável e `ProjectCore` como a fronteira externa consumida pela interface e pelos testes.

## Answer

Aprovado pelo responsável do produto em 2026-08-03 após o grilling das operações, autorizações, terminais e efeitos sobre a Sessão. O contrato detalhado foi registrado em [Contrato público de persistência do ProjectCore](../../../design/0015-contrato-publico-de-persistencia-do-project-core.md).

O nome inicial da pergunta foi esclarecido: `ProjectStore` continua concreto, interno e proprietário de DTOs, migrações, baseline e Publicação atômica; a superfície pública é `ProjectCore`. Ela oferece três operações em dois modos: criar editável, abrir editável e carregar revisão persistida somente para leitura. As duas primeiras devolvem um único `EditableProject`, que mantém juntos `ProjectSession`, pathname nativo, lease cross-process por Identidade, `PersistedBaseline` com sua trava física e estado técnico de migração. Serializar o documento e confirmar manualmente a Revisão salva deixam de ser operações públicas.

A criação recebe do diálogo do Windows uma autorização imutável `CreateOnly` ou `ReplaceConfirmed`. A segunda permite substituir um arquivo existente depois do aviso do sistema operacional; a primeira recusa um destino concorrente. Nenhuma permite quebrar uma Sessão existente, que retorna `ProjectInUse`. A criação só devolve Sessão depois de publicar, verificar e bloquear o arquivo completo com revisão inicial `0`.

O Salvamento recebe a revisão esperada, congela a candidata sob comandos serializados e distingue pedido obsoleto de conflito externo. `ProjectStore` só produz um recibo privado após comprovar identidade física, baseline exato, Publicação e bytes finais; `ProjectCore` consome esse recibo para confirmar `savedRevision`. Os terminais são `Saved`, `AlreadyCurrent`, `StaleRevision`, conflito de baseline, falha conclusiva e resultado inconclusivo que invalida a Sessão de forma fechada. Mudança criativa e migração técnica pendente permanecem separadas.

Cancelamentos ocorrem antes de o núcleo iniciar I/O. Fechar com mudanças oferece salvar, descartar ou cancelar; conflito e falha segura mantêm a Janela aberta, enquanto resultado inconclusivo encerra a edição. Criação, abertura, carga e Salvamento possuem erros próprios, transportados pela fronteira Tauri como códigos e dados estruturados, nunca como strings genéricas.
