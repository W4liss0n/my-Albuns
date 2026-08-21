# My Albuns

Documentação de produto para um programa de diagramação de álbuns.

## Ordem de autoridade

ADRs aceitos governam decisões difíceis de reverter; a especificação governa o comportamento observável; os designs detalham contratos de áreas específicas; e os tickets definem entregas verificáveis. Uma fonte de nível inferior pode acrescentar detalhe, nunca contradizer a superior — diante de um conflito, a implementação para até que o documento proprietário seja reconciliado.

O glossário é normativo apenas para o significado dos termos. As pesquisas são material técnico não normativo e não substituem nenhuma dessas fontes.

## Especificação e domínio

- [Especificação funcional](docs/specs/programa-de-diagramacao-de-albuns.md) — fonte canônica do comportamento observável do produto.
- [Glossário do domínio](CONTEXT.md) — nomes e significados dos conceitos; não especifica fluxos, algoritmos, interface ou critérios de aceite.

## Decisões arquiteturais

- [ADR 0001 — Vincular arquivos de mídia](docs/adr/0001-vincular-arquivos-externos.md) · aceito
- [ADR 0002 — Identidade de cópias externas](docs/adr/0002-identificar-copias-externas.md) · aceito
- [ADR 0003 — Limpeza de saídas órfãs pela nomeação](docs/adr/0003-limpar-saidas-orfas-pela-nomeacao.md) · aceito
- [ADR 0004 — Margens dentro da dimensão exportada](docs/adr/0004-manter-margens-dentro-da-dimensao-exportada.md) · aceito
- [ADR 0005 — Tauri 2, React/TypeScript e Rust com host por Projeto](docs/adr/0005-adotar-tauri-react-rust.md) · aceito
- [ADR 0006 — Publicação com transação limitada](docs/adr/0006-publicar-exportacao-com-transacao-limitada.md) · aceito
- [ADR 0007 — Caminhos Windows e identidade física](docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md) · aceito
- [ADR 0008 — Arranjo de reserva de Layout](docs/adr/0008-garantir-layout-compativel-por-arranjo-de-reserva.md) · aceito
- [ADR 0009 — Arquivo `.myalbuns` JSON versionado](docs/adr/0009-adotar-arquivo-myalbuns-json-versionado.md) · aceito

## Designs

Contratos detalhados por área. Os de interface descrevem superfícies e interação; os de núcleo descrevem organização de dados e propriedade de estado.

### Interface

- [0001 — Estrutura da Janela do Projeto](docs/design/0001-estrutura-da-janela-do-projeto.md) — Canvas contínuo, Barra da Lâmina, Painel de Layouts, Modo de edição, Painel de imagens e Painel contextual.
- [0002 — Tela de Boas-vindas](docs/design/0002-tela-de-boas-vindas.md) — ponto de entrada do aplicativo e sua relação com as Janelas de Projeto.
- [0003 — Criação de Projeto](docs/design/0003-criacao-de-projeto.md) — etapas `Dimensões` e `Personalização` e o diálogo nativo de Nome e Localização.
- [0004 — Exportação normal](docs/design/0004-exportacao-normal.md) — escopo, modo, formato, destino, preparação e publicação.
- [0005 — Tela de Problemas](docs/design/0005-tela-de-problemas.md) — superfície tabular reutilizável de bloqueios, conflitos e resultados.
- [0006 — Configuração da Exportação em lote](docs/design/0006-configuracao-da-exportacao-em-lote.md) — seleção, pré-validação e entrada no Modo de lote exclusivo.
- [0007 — Progresso de operações](docs/design/0007-progresso-de-operacoes.md) — representação única de progresso determinado e indeterminado.
- [0008 — Configuração da Geração em lote](docs/design/0008-configuracao-da-geracao-em-lote.md) — Projeto modelo, origem, destino e verificação prévia.
- [0009 — Configurações do aplicativo](docs/design/0009-configuracoes-do-aplicativo.md) — abas `Desempenho` e `Photoshop`, e o catálogo de comandos.

### Núcleo

- [0010 — Armazenamento local e Cache](docs/design/0010-armazenamento-local-e-cache.md) — organização de `%APPDATA%` e `%LOCALAPPDATA%`, metadados, invalidação e liberação de espaço.
- [0011 — Resolução e política de caminhos](docs/design/0011-resolucao-e-politica-de-caminhos.md) — caminhos locais, UNC, unidades mapeadas, caminhos longos e vínculos temporários por tentativa.
- [0012 — Propriedade de estado e módulos do núcleo](docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md) — proprietários do estado criativo, composição, mídias, Cache, Exportação e exclusividade operacional.
- [0013 — Contrato do Arquivo de Projeto v1](docs/design/0013-contrato-do-arquivo-de-projeto-v1.md) — envelope `.myalbuns`, DTO fechado, caminhos reversíveis, evolução e casos dourados.
- [0014 — Contrato JPEG do primeiro fluxo](docs/design/0014-contrato-jpeg-do-primeiro-fluxo.md) — composição, fontes aceitas, rasterização e publicação JPEG do primeiro corte produtivo.
- [0015 — Contrato público de persistência do ProjectCore](docs/design/0015-contrato-publico-de-persistencia-do-project-core.md) — autoridade de Identidade, abertura, Salvamento atômico e `Salvar como`.
- [0016 — Contrato do Arquivo de Projeto v2](docs/design/0016-contrato-do-arquivo-de-projeto-v2.md) — DTO fechado legado, `MediaRef` de Foto/Decorativo e etapa de migração v1 → v2.
- [0017 — Contrato da primeira composição com Foto](docs/design/0017-contrato-da-primeira-composicao-com-foto.md) — esquema v3, Frames, enquadramento, resolução do alvo de soltura e autoridade do Original na Exportação.

## Pesquisas

Material técnico não normativo, conservado como histórico das avaliações.

- [0001 — Plataforma e arquitetura](docs/research/0001-plataforma-e-arquitetura.md) · substituída
- [0002 — Alternativas com Rust e C#](docs/research/0002-alternativas-rust-csharp.md) · histórica
- [0003 — Fronteira entre interface C# e motor Rust](docs/research/0003-fronteira-csharp-rust.md) · alternativa histórica
- [0004 — Caminhos Windows e UNC](docs/research/0004-caminhos-windows-e-unc.md) · vigente

## Processo

- [AGENTS.md](AGENTS.md) — orientação para agentes que trabalham no repositório.
- [Docs de domínio](docs/agents/domain.md) — propriedade normativa das fontes e regras de vocabulário.
- [Issue tracker](docs/agents/issue-tracker.md) — convenções dos tickets em Markdown local.
- [Rótulos de triagem](docs/agents/triage-labels.md) — estados canônicos de um ticket.

## Entregas

O mapa de implementação está em [`.scratch/programa-diagramacao/`](.scratch/programa-diagramacao/), com um ticket de fatia vertical por arquivo sob [`issues/`](.scratch/programa-diagramacao/issues/). As arestas de bloqueio definem a fronteira de trabalho.

## Estado do repositório

Windows 10/11 x64 é o escopo inicial. No Windows 11 x64, o spike executável validou WebGL2 com backend de hardware verificável e Tauri 2 com React/TypeScript e Rust. A arquitetura aceita usa `MyAlbuns.exe` como processo global leve, um host independente por Projeto e um Processador de Imagens separado, conforme o [ADR 0005](docs/adr/0005-adotar-tauri-react-rust.md).
