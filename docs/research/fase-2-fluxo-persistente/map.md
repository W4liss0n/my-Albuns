---
status: historical
document: technical-research
ticket: fase-2-fluxo-persistente
date: 2026-08-03
updated: 2026-08-05
---

# Fase 2 — Primeiro fluxo real persistente

## Destination

Deixar sem decisões ocultas o caminho para criar um Projeto real em local escolhido, alterar seu DPI com Undo/Redo, salvar, fechar, reabrir e exportar a Lâmina visível como JPEG.

## Notes

- Este esforço usa o Wayfinder para planejar; seus tickets resolvem decisões e provas de arquitetura, não implementam o destino.
- A especificação canônica, os ADRs aceitos, `CONTEXT.md`, os designs e o mapa em `.scratch/programa-diagramacao/` continuam normativos em sua ordem de autoridade. Este mapa apenas refina a rota até `Esqueleto ponta a ponta` sem duplicar seus critérios.
- Sessões de decisão devem usar `grilling` e `domain-modeling`; sessões de protótipo devem usar `prototype`; pesquisa externa deve usar `research` e fontes primárias.
- A Fase 1 já fixou Tauri 2, React/TypeScript, Rust, PixiJS/WebGL2, um host independente por Projeto, um Processador de Imagens separado, `ProjectSession` como único proprietário criativo mutável e `CompositionCore` compartilhado por prévia e Exportação.
- `MyAlbuns2` permanece o namespace temporário durante esta fase. Sua migração não pode bloquear o primeiro fluxo nem ser tratada como concluída.
- Os tickets amplos `Renderizador final` e `Arquitetura de UI, mapa de telas e interação do editor` excedem este destino. A rota usa somente os cortes necessários para o Projeto v1, a Lâmina visível em JPEG e as telas já definidas de Boas-vindas, criação, editor e Exportação.

## Decisions so far

<!-- As respostas pertencem aos tickets resolvidos. Registrar aqui apenas uma síntese e o link. -->

- [Reconciliar a fronteira de caminhos da próxima fase](issues/01-reconciliar-a-fronteira-de-caminhos.md) — separou a fundação já provada, a integração necessária antes do primeiro fluxo e a migração final de `MyAlbuns2`, removendo o bloqueio artificial sem declarar trabalho ausente como concluído.
- [Delimitar o contrato JPEG verificável desta fase](issues/02-delimitar-o-contrato-jpeg-da-fase.md) — recomendou um gate JPEG/JFIF baseline, opaco, RGB 8-bit e sRGB, com DPI e dimensões determinísticos, mantendo cor ampla, demais formatos e Exportação completa fora desta fase.
- [Provar o codec reversível de caminhos do Projeto](issues/03-provar-o-codec-de-caminhos-do-projeto.md) — comprovou que o DTO nativo marcado como `windowsUtf16` preserva sem perda caminhos locais, UNC, mapeados, verbatim e não Unicode, mantendo pathname fora do frontend e separado da Identidade do Projeto.
- [Provar o Salvamento atômico preservando a trava do Projeto](issues/04-provar-o-salvamento-atomico-com-trava.md) — descartou a transferência direta de travas e comprovou a sequência com barreira estável por Identidade, verificação exata e encerramento fail-closed em resultados inconclusivos.
- [Materializar o bootstrap entre Boas-vindas e Host do Projeto](issues/05-materializar-o-bootstrap-do-host-de-projeto.md) — aprovou um bootstrap descartável de uma requisição e um terminal correlacionado, no qual o Host cria ou abre a única sessão e sobrevive à saída do processo global.
- [Decidir o contrato do arquivo de Projeto v1](issues/06-decidir-o-contrato-do-arquivo-de-projeto-v1.md) — fixou `.myalbuns` como JSON UTF-8 estrito e versionado, com DTO v1 fechado, caminhos Windows reversíveis, migrações em memória e separação entre `ProjectStore` e domínio.
- [Decidir o contrato JPEG do primeiro fluxo](issues/07-decidir-o-contrato-jpeg-do-primeiro-fluxo.md) — fixou `Exportar Lâmina` como JPEG real da revisão visível, com composição compartilhada, fontes originais sRGB controladas, segurança de recursos e Publicação tipada.
- [Fechar o contrato público do ProjectStore](issues/08-fechar-o-contrato-publico-do-project-store.md) — manteve `ProjectStore` interno e fechou no `ProjectCore` criação substituível autorizada, exclusão editável por Identidade e arquivo físico, carga somente leitura e Salvamento com revisão esperada, prova privada e terminais tipados.
- [Provar a fronteira pública do fluxo multiprocesso](issues/09-provar-a-fronteira-publica-do-fluxo.md) — aprovou o harness mínimo em que `ProjectCore`, Host sobrevivente, snapshot visível, `ExportPipeline` e Processador atravessam o fluxo real com tentativas e cancelamentos observáveis.

## Not yet specified

- Nenhuma decisão restante dentro deste destino. A próxima atividade é decompor `Esqueleto ponta a ponta` em tickets de implementação executáveis, preservando os contratos resolvidos neste mapa.

## Out of scope

- Migrar `%APPDATA%` e `%LOCALAPPDATA%` de `MyAlbuns2` para `MyAlbuns`; isso permanece adiado até a finalização completa do programa.
- Concluir todo o escopo de PNG, PDF, perfis profissionais de cor, intervalos, múltiplas saídas e lote do `Renderizador final`.
- Concluir todo o mapa de interação de edição avançada, layouts, lote e Configurações abrangido pelo ticket amplo de UI.
- `Salvar como`, Cópia externa completa, movimentação, Recuperação, foco por alias e múltiplos Projetos.
- Edição completa de Frames e Fotos, ciclo completo de mídias, Gerador de Layouts, Photoshop e operações em lote.
