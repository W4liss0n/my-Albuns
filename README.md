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
- [0003 — Criação de Projeto](docs/design/0003-criacao-de-projeto.md) — etapas `Configurações` e `Personalização` e o diálogo nativo de Nome e Localização.
- [0004 — Exportação normal](docs/design/0004-exportacao-normal.md) — escopo, modo, formato, destino, preparação e publicação.
- [0005 — Tela de Problemas](docs/design/0005-tela-de-problemas.md) — superfície tabular reutilizável de bloqueios, conflitos e resultados.
- [0006 — Configuração da Exportação em lote](docs/design/0006-configuracao-da-exportacao-em-lote.md) — seleção, pré-validação e entrada no Modo de lote exclusivo.
- [0007 — Progresso de operações](docs/design/0007-progresso-de-operacoes.md) — representação única de progresso determinado e indeterminado.
- [0008 — Configuração da Geração em lote](docs/design/0008-configuracao-da-geracao-em-lote.md) — Projeto modelo, origem, destino e verificação prévia.
- [0009 — Configurações do aplicativo](docs/design/0009-configuracoes-do-aplicativo.md) — abas `Desempenho` e `Photoshop`, e o catálogo de comandos.
- [0018 — Mapa de navegação e interação da interface](docs/design/0018-mapa-de-navegacao-e-interacao-da-interface.md) — superfícies, nomes atuais, modos, transições, ownership e registro de validação visual.

### Núcleo

- [0010 — Armazenamento local e Cache](docs/design/0010-armazenamento-local-e-cache.md) — organização de `%APPDATA%` e `%LOCALAPPDATA%`, metadados, invalidação e liberação de espaço.
- [0011 — Resolução e política de caminhos](docs/design/0011-resolucao-e-politica-de-caminhos.md) — caminhos locais, UNC, unidades mapeadas, caminhos longos e vínculos temporários por tentativa.
- [0012 — Propriedade de estado e módulos do núcleo](docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md) — proprietários do estado criativo, composição, mídias, Cache, Exportação e exclusividade operacional.
- [0013 — Contrato do Arquivo de Projeto v1](docs/design/0013-contrato-do-arquivo-de-projeto-v1.md) — envelope `.myalbuns`, DTO fechado, caminhos reversíveis, evolução e casos dourados.
- [0014 — Contrato JPEG do primeiro fluxo](docs/design/0014-contrato-jpeg-do-primeiro-fluxo.md) — composição, fontes aceitas, rasterização e publicação JPEG do primeiro corte produtivo.
- [0015 — Contrato público de persistência do ProjectCore](docs/design/0015-contrato-publico-de-persistencia-do-project-core.md) — autoridade de Identidade, abertura, Salvamento atômico e `Salvar como`.
- [0016 — Contrato do Arquivo de Projeto v2](docs/design/0016-contrato-do-arquivo-de-projeto-v2.md) — DTO fechado legado, `MediaRef` de Foto/Decorativo e etapa de migração v1 → v2.
- [0017 — Contrato da primeira composição com Foto](docs/design/0017-contrato-da-primeira-composicao-com-foto.md) — esquema v3, Frames, enquadramento, resolução do alvo de soltura e autoridade do Original na Exportação.
- [0019 — Contrato do Renderizador final](docs/design/0019-contrato-do-renderizador-final.md) — composição canônica, JPEG/PNG/PDF, captura estável dos Originais, Publicação e corpus dourado.
- [0020 — Importação com decode único e lotes](docs/design/0020-importacao-com-decode-unico-e-lotes.md) — tentativa com caminhos congelados, adoção pelo Monitor, transações de Cache e capacidade de CPU/RAM.

## Pesquisas

O [fechamento da importação e das miniaturas](docs/research/2026-09-06-fechamento-importacao-e-miniaturas.md) reúne o resultado final, a organização do código, as validações e o executável local de referência. Os relatórios abaixo conservam as medições e pendências observadas em cada etapa; o fechamento registra o estado consolidado deste ciclo.

- [Importação de várias Fotos JPEG](docs/research/2026-09-05-importacao-multipla-jpeg.md) — registro da implementação inicial do recorte do #19, com resultado parcial e uma ação de Histórico.
- [Reuso da inspeção e do Cache](docs/research/2026-09-06-reuso-da-inspecao-e-do-cache-na-importacao.md) — reaproveitamento da validação entre importação, Monitor e primeira demanda de prévias.
- [Integração das otimizações de importação](docs/research/2026-09-06-integracao-importacao-producao.md) — decode único, lotes, orçamento de recursos e evidência do percurso com o Processador real.
- [Progresso durante a importação](docs/research/2026-09-06-progresso-importacao.md) — avanço por imagem enquanto os lotes estão ativos, incluindo recuperação e falhas parciais.
- [Retenção de prévias ao rolar](docs/research/2026-09-06-retencao-previas-painel.md) — cache recente com limites de memória e reutilização das miniaturas já visitadas.
- [Abertura e primeira rolagem do Painel](docs/research/2026-09-06-abertura-e-primeira-rolagem-do-painel.md) — demanda inicial pela geometria, recuperação de prévias e antecipação das próximas linhas.

Material técnico não normativo, conservado como histórico das avaliações.

- [0001 — Plataforma e arquitetura](docs/research/0001-plataforma-e-arquitetura.md) · substituída
- [0002 — Alternativas com Rust e C#](docs/research/0002-alternativas-rust-csharp.md) · histórica
- [0003 — Fronteira entre interface C# e motor Rust](docs/research/0003-fronteira-csharp-rust.md) · alternativa histórica
- [0004 — Caminhos Windows e UNC](docs/research/0004-caminhos-windows-e-unc.md) · vigente

## Processo

- [AGENTS.md](AGENTS.md) — orientação para agentes que trabalham no repositório.
- [Docs de domínio](docs/agents/domain.md) — propriedade normativa das fontes e regras de vocabulário.
- [Issue tracker](docs/agents/issue-tracker.md) — convenções dos tickets no GitHub.
- [Rótulos de triagem](docs/agents/triage-labels.md) — estados canônicos de um ticket.

## Entregas

O mapa de implementação está no [GitHub, issue #1](https://github.com/W4liss0n/my-Albuns/issues/1), com tickets de entrega e seus bloqueadores. Os arquivos antigos em `.scratch/programa-diagramacao/` são históricos.

O [primeiro recorte da edição de Frames](docs/research/2026-09-07-geometria-de-frame-unico.md), integrado pela [PR #66](https://github.com/W4liss0n/my-Albuns/pull/66), acrescenta movimento e redimensionamento de um Frame no Modo de edição da Lâmina. A [seleção múltipla](docs/research/2026-09-07-geometria-de-selecao-multipla.md), integrada pela [PR #67](https://github.com/W4liss0n/my-Albuns/pull/67), estende os gestos ao grupo. A [ordenação da Pilha visual](docs/research/2026-09-07-ordenacao-de-frames.md) acrescenta os quatro comandos de Organizar, menu contextual e atalhos para avançar/recuar. O núcleo preserva geometria, ordem e ajustes no Histórico, Salvamento e reabertura. A issue #20 continua aberta para os demais comandos de Frames.

A entrega anterior é o [fechamento da importação e das miniaturas, de 6 de setembro de 2026](docs/research/2026-09-06-fechamento-importacao-e-miniaturas.md): importação múltipla de JPEGs, processamento em lotes, progresso contínuo e carregamento/retenção das prévias. O código foi validado localmente e o usuário confirmou a correção final. A [PR #65](https://github.com/W4liss0n/my-Albuns/pull/65) reúne essa entrega sobre `main`, após a integração da [PR #64](https://github.com/W4liss0n/my-Albuns/pull/64). O aceite usa a CI e a validação manual, conforme a exceção registrada na política de validação. Este fechamento cobre o recorte implementado, não todo o escopo da issue #19.

A [revisão de 5 de setembro de 2026](docs/research/2026-09-05-estado-do-produto-e-proxima-entrega.md) conserva o diagnóstico anterior a essa entrega e as demais pendências do produto.

## Estado do repositório

Windows 10/11 x64 é o escopo inicial. No Windows 11 x64, o spike executável validou WebGL2 com backend de hardware verificável e Tauri 2 com React/TypeScript e Rust. A arquitetura aceita usa `MyAlbuns.exe` como processo global leve, um host independente por Projeto e um Processador de Imagens separado, conforme o [ADR 0005](docs/adr/0005-adotar-tauri-react-rust.md).

## Toolchain de desenvolvimento

O Rust está fixado na versão exata declarada em [`rust-toolchain.toml`](rust-toolchain.toml), incluindo `clippy` e `rustfmt`. `npm run setup:local` instala essa versão dentro de `.tools/`, também em checkouts que já possuíam uma instalação local, e a torna o padrão do `rustup` local. Os comandos Rust do repositório devem ser executados pelos scripts `npm run check:rust`, `npm run test:rust` e `npm run quality:rust`, que selecionam a mesma versão fixada.

## Validação durante o desenvolvimento

`npm run validate` é o comando padrão: prepara o Processador de Imagens e executa
build, contratos, tipos, testes
React, testes da automação e verificações Rust sem abrir o MyAlbuns. O relatório
e os logs por etapa ficam em `.tools/validation/`. Durante uma edição, os comandos
de teste focados continuam disponíveis; não é necessário repetir a suíte inteira.

A captura `npm run ui:acceptance` também usa navegador sem janela. Selecione
somente os estados afetados com `MYALBUNS_UI_SCENARIO_IDS`; a aprovação visual
continua dependendo da revisão das capturas.

Os testes com janelas ficam separados. O workflow **Validation** executa a
validação sem janelas automaticamente nas PRs. O piloto nativo de Cópia externa
fica disponível para execução manual, informando em `native_runner` o rótulo de
um runner Windows x64 hospedado com WebGL2 por hardware confirmado. Deixar o campo
vazio executa somente a validação sem janelas.

O [piloto no runner comum `windows-2022`](https://github.com/W4liss0n/my-Albuns/actions/runs/33935287054)
confirmou que o aplicativo entra em modo seguro porque esse ambiente não conseguiu
criar WebGL2 por hardware. Por isso, o piloto permanece fora da rotina automática
e ainda não está aprovado. Seus logs e capturas ficam retidos como artefatos.
A execução manual pelo GitHub exige que o workflow esteja na branch padrão.
O piloto não aprova a jornada completa nem substitui a verificação de GPU no
hardware final.

Para um ambiente Windows reservado aos testes, `npm run build:native-tests`
prepara uma única compilação com hashes e commit. O comando
`npm run test:native-owned-dialogs -- -Scenario external-copy-opening-owner`
seleciona somente esse cenário; `late-graphics-project-dialog` seleciona o outro.
O binário é reaproveitado enquanto o commit, a fonte limpa e os hashes coincidirem.
Execução local com janelas exige combinar o uso da área de trabalho e acrescentar
`-AllowVisibleWindows`. A [política de validação](docs/agents/native-ui-gates.md)
detalha os limites de cada prova e a jornada legada ainda pendente.


Para investigar apenas o fechamento após Salvar como, use
`npm run test:native-project-close` no ambiente reservado. O cenário altera e
salva original e cópia em Hosts distintos, fecha o original uma única vez e
confirma a permanência da cópia antes de limpar os processos. Ele usa o mesmo
build verificado e a mesma autorização dos demais gates. Em caso de falha,
conserva a etapa interrompida, os registros, o estado das janelas e as capturas
possíveis em `.scratch/project-close-evidence/`. Sua preparação e os testes da
automação não constituem aprovação do fechamento nativo.

Em 7 de setembro de 2026, o usuário confirmou o fechamento manual e autorizou a
integração das PRs #64 e #65 com a CI aprovada. A investigação automatizada desse
fechamento está suspensa e só deve voltar a abrir janelas mediante nova
solicitação explícita. A [exceção de integração](docs/agents/native-ui-gates.md#user-approved-integration-exception-2026-09-07)
registra esse aceite sem apresentar os ensaios inconclusivos como aprovados.
