# 01 — Expandir o ProjectCore com o Documento v1

**What to build:** introduzir, ao lado do scaffolding da Fase 1, o contrato produtivo mínimo de documento e persistência que permitirá criar, abrir e carregar um Projeto `.myalbuns` v1 somente pela superfície pública do `ProjectCore`.

**Blocked by:** None — can start immediately.

**Type:** implementation

**Status:** resolved

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [Contrato do Arquivo de Projeto v1](../../../docs/design/0013-contrato-do-arquivo-de-projeto-v1.md); [contrato público de persistência do ProjectCore](../../../docs/design/0015-contrato-publico-de-persistencia-do-project-core.md); [decisão do arquivo v1](../../fase-2-fluxo-persistente/issues/06-decidir-o-contrato-do-arquivo-de-projeto-v1.md); [decisão da superfície pública](../../fase-2-fluxo-persistente/issues/08-fechar-o-contrato-publico-do-project-store.md).

- [x] O novo documento é um JSON UTF-8 estrito sem BOM, identificado por `documentType: "myalbuns.project"`, `schemaVersion: 1`, UUID v4 canônico e Revisão inicial `0`, com envelope e DTO fechado conforme o contrato v1.
- [x] O documento guarda somente estado persistente do Projeto; Nome, Localização, Undo/Redo, indicação de mudanças pendentes, estado da interface, Cache, Recuperação e resultados de Exportação não entram no arquivo.
- [x] O estado v1 contém exatamente Documento físico, padrões globais de Background, Overlay e Borda, referências ordenadas a Decorativos e ordem, Identidade e lados ativos das Lâminas; medidas físicas são micrômetros inteiros e a Unidade é apenas apresentação.
- [x] Todo caminho Windows persistido usa somente o DTO reversível marcado como `windowsUtf16`; pathname, Identidade do Projeto e identidade física permanecem conceitos distintos.
- [x] O mapeamento entre DTO e domínio preserva tipos separados, valida invariantes depois da leitura e não permite que os tipos Serde se tornem o modelo mutável da Sessão.
- [x] A superfície produtiva do `ProjectCore` oferece somente criar Projeto editável, abrir Projeto editável e carregar revisão persistida somente leitura; `ProjectStore`, serialização, migração e confirmação de Revisão salva continuam internos.
- [x] Criar o fixture neutro produz duas Lâminas duplas de 60 × 30 cm, 300 DPI, Sangria e segurança de 3 mm, Background branco e nenhuma mídia, Overlay, borda ou Frame.
- [x] Leitura rejeita campos ausentes, desconhecidos ou duplicados, tipo incorreto, schema futuro ou legado não suportado, UUID inválido, referências quebradas e estado de Álbum inválido com resultados estruturados; os formatos demonstrativos da Fase 1 não ganham compatibilidade pública.
- [x] Invariantes cobrem DPI `1..=1200`, largura positiva e par em micrômetros, dimensões raster estruturais entre `1` e `65.535` pixels por eixo e Sangria/segurança que preservem áreas positivas.
- [x] O Álbum exige ao menos duas Lâminas com IDs únicos: a primeira aceita `both` ou `right`, a última `both` ou `left` e todas as internas são `both`.
- [x] Background, Overlay e Borda são uniões fechadas, cores usam `#RRGGBB` canônico e todo `mediaId` referenciado existe uma única vez sem pathname duplicado.
- [x] Versões suportadas migram somente em memória; carga não escreve na origem e este ticket não cria importador para `.myalbum`, `schemaVersion: 3` ou formatos dos spikes.
- [x] Falhas públicas distinguem no mínimo documento de tipo inválido, schema futuro, schema legado, documento malformado, estado inválido e caminho inválido sem criar Sessão.
- [x] A expansão convive temporariamente com os consumidores demonstrativos existentes sem duplicar regras de domínio nem tornar público um `Store<T>` genérico.
- [x] Testes pela superfície do `ProjectCore` cobrem os casos dourados e negativos do documento v1, inclusive round-trip dos caminhos nativos e carga somente leitura sem criar uma Sessão editável.
- [x] Boas-vindas, diálogos, bootstrap e Host permanecem fora deste ticket; os consumidores posteriores utilizam a fronteira expandida sem copiar codec ou validação.

## Comments

- 2026-08-03 — Implementado o contrato produtivo v1 pela fronteira pública do `ProjectCore`, mantendo domínio, DTO/codec, sessão persistente, armazenamento físico e coordenação de identidade em módulos separados.
- Evidência externa: 21 testes em `project_document_v1`, incluindo documento neutro e completo, esquema fechado, invariantes, caminhos Windows reversíveis, criação, substituição, bloqueios e classificação física de Cópia externa sem escrita na origem.
- Gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npm test` e `npm run build` concluídos com sucesso.
- Limites preservados: integração do Host fica no ticket 02; contenção do filho e hardening de corridas de criação ficam no ticket 03; Salvamento e baseline concorrente ficam no ticket 07; remoção do scaffolding demonstrativo fica no ticket 11.
