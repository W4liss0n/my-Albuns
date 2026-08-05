---
status: historical
document: technical-research
ticket: fase-2-fluxo-persistente-03
date: 2026-08-03
updated: 2026-08-05
---

# Provar o codec reversível de caminhos do Projeto

Type: prototype

Status: resolved

Blocked by: None

## Question

Qual representação persistida permite o round-trip sem perda dos caminhos Windows aceitos pelo produto, incluindo disco local, UNC, unidade mapeada e formas verbatim, sem transportar o caminho pelo frontend, convertê-lo em Identidade ou normalizá-lo textualmente?

O protótipo deve comparar a representação escolhida com os contratos nativos já provados, produzir casos dourados e deixar claro como o DTO entra no documento v1.

## Prototype

- Branch: `codex/prototype-project-path-codec`
- Commit: `677626f`
- Worktree: `C:\Users\Usuario\AppData\Local\Temp\my-albuns-prototype-path-codec`
- Run: `npm run prototype:path-codec`
- Validação: aceita pelo responsável pelo produto em 2026-08-03.

## Answer

O documento v1 deve persistir cada caminho nativo por meio do DTO marcado já usado pela fronteira Rust, com `encoding: "windowsUtf16"` e as unidades UTF-16 originais. O protótipo comprovou round-trip exato para disco local, UNC, unidade mapeada, `VerbatimDisk`, `VerbatimUNC` e para um caminho contendo surrogate não pareado.

Uma string Unicode comum não é uma representação reversível: no caso não Unicode ela substitui a unidade inválida e perde informação. O codec permanece nativo; o frontend não transporta nem normaliza o pathname. A Identidade do Projeto continua sendo um campo independente e permaneceu inalterada depois da serialização e reabertura.

O resultado decide apenas a representação dos campos de caminho. Extensão, envelope e schema completo do documento continuam pertencendo ao ticket `Decidir o contrato do arquivo de Projeto v1`.
