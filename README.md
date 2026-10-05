# My Albuns

Aplicativo de diagramação de álbuns para Windows 10/11 x64, feito com Tauri 2, React/TypeScript e Rust. `MyAlbuns.exe` é um processo global leve; cada Projeto abre em um Host próprio, e um Processador de Imagens separado trata as imagens, conforme o [ADR 0005](docs/adr/0005-adotar-tauri-react-rust.md).

## Documentação

- [Mapa da documentação](docs/README.md) — ordem de autoridade, pastas e por onde começar em cada assunto.
- [Especificação funcional](docs/specs/programa-de-diagramacao-de-albuns.md) — fonte canônica do comportamento observável do produto.
- [Glossário do domínio](CONTEXT.md) — nomes e significados dos conceitos.
- [AGENTS.md](AGENTS.md) — orientação para agentes que trabalham no repositório.
- [CODING_STANDARDS.md](CODING_STANDARDS.md) — critérios de revisão.
- [Issue #1 no GitHub](https://github.com/W4liss0n/my-Albuns/issues/1) — mapa de implementação, com os tickets de entrega e seus bloqueadores.

## Pastas do repositório

| Pasta | Conteúdo |
|---|---|
| `src/` | interface em React/TypeScript: uma pasta por janela (`global/`, `project-dialog/`, `settings/`, `batch-export/`, `generation/`, `image-viewer/`, `dialog/`), a janela do Projeto em `project/` (uma subpasta por área: `canvas/`, `sheets/`, `media-panel/`, `inspector/`, `layouts/`, `workspace/`, `editor/`), e as camadas compartilhadas (`domain/`, `contracts/`, `application/`, `platform/`, `ui/`, `state/`) |
| `previews/` e `src/previews/` | prévias e regressões visuais, usadas só no desenvolvimento |
| `src-tauri/` | Host desktop em Rust (Tauri) |
| `crates/` | núcleo em Rust: `myalbuns-core`, `myalbuns-imaging`, `myalbuns-imaging-protocol`, `myalbuns-paths` e `myalbuns-logging` |
| `scripts/` | toolchain local, contratos, validação, testes com janelas e aceitação visual |
| `tests/fixtures/` | casos compartilhados pelos testes em Rust e TypeScript |
| `public/models/` | modelo e WASM do MediaPipe usados na correção de olhos |
| `resources/windows/` | manifesto Windows dos executáveis |
| `docs/` | documentação do produto ([mapa](docs/README.md)) |
| `.interface-design/` | direção visual aceita |
| `.out-of-scope/` | melhorias recusadas de forma duradoura |
| `benchmark-data/` | fotos locais para medições, fora do Git |
| `.tools/` e `.scratch/` | toolchain local e área de trabalho local, fora do Git; regras em [AGENTS.md](AGENTS.md#local-folders) |

Os seis arquivos HTML da raiz são as páginas das janelas do aplicativo e as entradas do `vite build`.

## Toolchain de desenvolvimento

O Rust está fixado na versão exata declarada em [`rust-toolchain.toml`](rust-toolchain.toml), incluindo `clippy` e `rustfmt`. `npm run setup:local` instala essa versão dentro de `.tools/`, também em checkouts que já possuíam uma instalação local, e a torna o padrão do `rustup` local. Os comandos Rust do repositório devem ser executados pelos scripts `npm run check:rust`, `npm run test:rust` e `npm run quality:rust`, que selecionam a mesma versão fixada.

## Validação durante o desenvolvimento

Para gerar a distribuição otimizada, execute `npm run tauri:build -- --ci`.
O comando compila o aplicativo e o Processador de Imagens em Release e produz
o instalador em `target/release/bundle/nsis/`. O perfil usa ThinLTO e uma unidade
de geração de código por crate; preserva a recuperação de falhas por `unwind`.
O Tauri remove comandos de plugins ausentes das permissões estáticas.
Essa configuração prioriza o tamanho e a execução do programa e pode aumentar
o tempo da compilação. A medição está na
[comparação de compilações Release](docs/research/2026-09-17-otimizacao-da-compilacao-release.md).

`npm run validate` é o comando padrão: confere formatação Rust e tipos, executa
testes React, testes da automação e as quatro regressões do Canvas em navegador
sem janela (gestos de Quadro, inserção de Fotos, troca de conteúdo entre Quadros
e isolamento de comandos entre janelas), prepara o Processador de Imagens e então
executa build, contratos e verificações Rust sem abrir o MyAlbuns. O relatório
e os logs por etapa ficam em `.tools/validation/`. Durante uma edição, os comandos
de teste focados continuam disponíveis; não é necessário repetir a suíte inteira.

Essa validação também executa `npm run test:owned-window-fitting` depois do
build. O teste carrega o diálogo compilado em navegador sem janela e aplica os
pedidos reais de tamanho a um viewport, verificando que a tabela de Problemas
permanece visível e consegue crescer novamente após o progresso. As evidências
ficam em `.scratch/ui-acceptance/owned-window-fitting/`.

A captura `npm run ui:acceptance` também usa navegador sem janela. Selecione
somente os estados afetados com `MYALBUNS_UI_SCENARIO_IDS`; a aprovação visual
continua dependendo da revisão das capturas.

`npm run test:photo-placement` verifica a inserção sucessiva de doze Fotos PNG/JPEG com
miniaturas já carregadas, usando o Canvas real em navegador sem janela. O teste
compara os pixels do primeiro desenho de cada Foto com os pixels estabilizados,
para detectar a aparição transitória do fundo provisório, e também confere a
rasterização das imagens SVG usadas nas fixtures. A abertura também é exercitada
antes de a URL da prévia chegar e durante o carregamento da textura: nenhum
desenho de demonstração deve ocupar a Foto nesses intervalos. Captura e resultados
ficam em `.scratch/ui-acceptance/photo-placement/`.

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

Para verificar a restauração da WebView em uma execução local autorizada, use
`npm run test:native-webview-restore -- -AllowVisibleWindows`. O teste abre sua
própria janela descartável, verifica a resposta da janela e da interface após
três ciclos de minimizar/restaurar e confere o tamanho no retorno e no
redimensionamento seguinte. Os registros ficam em `.scratch/webview-restore-gate/`.


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
