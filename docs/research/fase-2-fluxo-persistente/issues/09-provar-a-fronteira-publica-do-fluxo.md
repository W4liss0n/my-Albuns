---
status: historical
document: technical-research
ticket: fase-2-fluxo-persistente-09
date: 2026-08-03
updated: 2026-08-05
---

# Provar a fronteira pública do fluxo multiprocesso

Type: prototype

Status: resolved

Blocked by: 05 — Materializar o bootstrap entre Boas-vindas e Host do Projeto; 07 — Decidir o contrato JPEG do primeiro fluxo; 08 — Fechar o contrato público do ProjectStore

## Question

Qual é o menor harness corrente que comprova criar Projeto, alterar DPI, desfazer e refazer, salvar, fechar, reabrir e exportar JPEG atravessando somente `ProjectCore` e `ExportPipeline`, inclusive diálogo e reinício do host, sem reconstruir o harness comparativo A/B arquivado da Fase 1?

O protótipo deve definir a fronteira automatizada, os observáveis e os comandos reproduzíveis que tornarão `Esqueleto ponta a ponta` implementável e verificável.

## Prototype

- Branch: `codex/prototype-public-flow-boundary`
- Commit: `8b2ae1f32574ee369e58d56fc3afea76f8674022`
- Worktree: `C:\Users\Usuario\AppData\Local\Temp\my-albuns-prototype-public-flow`
- Run: `npm run prototype:public-flow`
- Evidência automatizada: jornada multiprocesso `14/14`; cancelamento anterior ao núcleo `4/4`; `cargo clippy --locked -p myalbuns-desktop --example prototype_public_flow_boundary -- -D warnings`; `cargo fmt --all -- --check`.
- Validação humana: aprovada pelo responsável pelo produto em 2026-08-03.

## Answer

A menor fronteira verificável é um harness descartável no qual `ProjectCore` mantém `ProjectStore` e `ProjectSession` internos, o processo global entrega uma única requisição correlacionada a um Host que sobrevive à sua saída e `ExportPipeline` recebe somente o `RenderSnapshot` visível e a Lâmina selecionada. O Processador recebe uma `ComposedOutputUnit` imutável e produz o JPEG real; um ledger exclusivo do instrumento torna tentativas e cancelamentos observáveis sem integrar esse protocolo ao produto.

A jornada aprovada comprovou criação `CreateOnly` com Identidade gerada pelo núcleo, alteração de DPI em Revisão com Undo/Redo, rejeição `StaleRevision`, Salvamento confirmado, exclusividade `ProjectInUse`, encerramento e reabertura em outro Host com Histórico vazio e Exportação da segunda Lâmina usando o estado visível ainda não salvo. Também comprovou PIDs separados, JPEG 7.087 × 3.543 a 300 DPI com sRGB2014, ausência de efeito da Exportação sobre Salvamento e Histórico e cancelamento antes de qualquer tentativa no núcleo.

O protótipo define a forma mínima e os observáveis para decompor `Esqueleto ponta a ponta`. Ele não é implementação produtiva, não substitui as matrizes já aprovadas e não deve ser mesclado; a branch registrada acima permanece sua fonte primária.
