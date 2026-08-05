---
status: historical
document: technical-research
ticket: fase-2-fluxo-persistente-06
date: 2026-08-03
updated: 2026-08-05
---

# Decidir o contrato do arquivo de Projeto v1

Type: grilling

Status: resolved

Blocked by: 03 — Provar o codec reversível de caminhos do Projeto

## Question

Qual será o contrato persistido do Projeto v1: extensão, identificação de tipo, envelope e formato, `schemaVersion`, Identidade, revisão confirmada, incorporação do codec de caminhos, validação estrutural e política de migrações sequenciais?

A decisão deve distinguir fixtures do spike de compatibilidade pública, fornecer exemplos válidos e inválidos e preservar `ProjectDomain` sem conhecimento de I/O, serialização ou versões antigas.

## Answer

Aprovado pelo responsável do produto em 2026-08-03. O [ADR 0009](../../../adr/0009-adotar-arquivo-myalbuns-json-versionado.md) e o [design do contrato v1](../../../design/0013-contrato-do-arquivo-de-projeto-v1.md) são a decisão normativa.

O Arquivo de Projeto usa a extensão `.myalbuns`, associação `Projeto MyAlbuns` no Windows e um único JSON UTF-8 estrito, sem BOM, identificado internamente por `myalbuns.project`. O contrato público começa em `schemaVersion: 1`; `.myalbum` e `schemaVersion: 3` dos spikes são descartáveis e não recebem compatibilidade. O envelope fechado contém `documentType`, `schemaVersion`, `projectId`, `revision` e `project`, rejeitando campos ausentes, desconhecidos ou duplicados.

`ProjectDocumentV1` persiste exatamente Documento físico, padrões globais de Background, Overlay e Borda, referências ordenadas a Decorativos externos e a ordem/identidade/lados ativos das Lâminas. Caminhos usam somente o DTO reversível `windowsUtf16`. O v1 não reserva campos para Frames, Fotos, aplicações locais, Cache, Recuperação, interface ou Histórico; o fixture neutro de duas Lâminas duplas, branco e sem mídia é válido e exportável.

A Identidade é UUID v4 canônico e a Revisão representa estado criativo, não concorrência. O `ProjectStore` conserva como prova um `PersistedBaseline` de identidade física, trava e bytes exatos. Uma Cópia externa editável é reidentificada no mesmo schema antes da Sessão, sob as barreiras antiga e nova; a entrada headless falha sem escrita e exige resolução interativa.

Cada versão pública tem DTO fechado e migrações puras de um passo. A abertura migra somente em memória e apenas `Salvar` explícito escreve a versão atual. `ProjectStore` possui JSON, DTOs, caminhos, I/O e migrações; `ProjectDomain` possui unicidade, referências e invariantes criativas. Erros estruturais, de caminho e de estado permanecem distinguíveis e os casos dourados fixam a matriz de extensões, duplicatas, limites, cópias e documentos válidos/inválidos.
