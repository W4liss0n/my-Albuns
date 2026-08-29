---
status: current
document: technical-research
ticket: 8-esqueleto-ponta-a-ponta
date: 2026-08-14
updated: 2026-08-28
---

# Jornada produtiva ponta a ponta

## Pergunta

Como provar que o primeiro fluxo completo usa somente as fronteiras produtivas
do `ProjectCore` e da `ExportPipeline`, em processos reais, sem conservar os
atalhos headless e os objetos de demonstração usados pelos gates anteriores?

## Gate adotado

`npm run test:productive-journey` constrói o aplicativo Tauri e o Processador,
abre a Tela Global real e dirige a interface pelo WebView2. A rodada usa um
diretório temporário curto porque o perfil WebView2 por Projeto acrescenta seu
namespace opaco e os próprios componentes internos do Chromium ao caminho.
Esse detalhe não muda o armazenamento normativo: o perfil continua sob
`State/WebView2`, conforme o design 0010.

A automação atravessa apenas ações públicas da interface e diálogos nativos.
Ela não invoca comandos Tauri diretamente e não depende de `cfg(test)` no
produto. Antes da janela real, o mesmo comando executa uma jornada pela API
pública do `ProjectCore`; essa fatia comprova `CreateOnly`, propriedade
editável exclusiva, DPI, Desfazer/Refazer, recusa de Revisão obsoleta sem I/O,
Salvamento e reabertura com Histórico vazio. O gate multiprocesso observa, em
ordem:

1. cancelamento do primeiro diálogo de criação sem arquivo e sem Host;
2. autorização `CreateOnly`, criação e handoff causal Global → Host;
3. criação de extremidades de página única, Background `#204060`,
   materialização do Canvas e seleção observada da segunda Lâmina;
4. DPI 300, Desfazer, Refazer e persistência da revisão 1;
5. DPI 360 ainda não salvo;
6. cancelamento do diálogo de Exportação antes da `ExportPipeline` e do
   Processador;
7. Exportação JPEG da segunda Lâmina dupla em 1.440 × 360 px, distinguível
   da primeira Lâmina de Página única em 720 × 360 px, com amostra do Background salvo e
   sem alterar o Projeto nem o histórico pendente;
8. fechamento com descarte, reabertura em outro Host e histórico vazio;
9. terminais exatamente correlacionados por PID para Global, Host e
   Processador, seguidos de cleanup completo.

O runner também lê o source público da WebView e falha se nele aparecer algum
caminho nativo da rodada. A captura do elemento Canvas precisa conter amostras
não brancas antes de a prova ser aceita.

O mesmo gate conserva a distinção entre camadas de prova. Antes da aplicação
real, ele executa pela fronteira pública do `ProjectWorkspace` a matriz de
Desfazer/Refazer atrasado seguida por Adicionar, Excluir ou Converter, nos
terminais de sucesso e falha. Essa camada também cobre cancelamento integral do
preview/drop durante mutação estrutural, escopo contextual de `Delete`, menu
contido na viewport, reordenação por Barra e Grade e política progressiva de
auto-scroll. Esses checks não são renomeados como WebView2: cada entrada do
recibo declara `proofLayer`.

O recibo também registra limites negativos de cobertura. A corrida determinista
de Histórico não recebe um hook temporal exclusivo de teste no produto; a
distância progressiva do auto-scroll não é tratada como propriedade de uma
captura estática; e a aceitação visual do menu não alega mostrar simultaneamente
as quatro geometrias de canto. As três propriedades continuam comprovadas por
testes públicos deterministas, mas não são atribuídas à camada visual ou à
rodada Windows além do que ela realmente observa.

## Reprodutibilidade

O wrapper captura `HEAD` e a árvore inteira antes e depois da execução, exclui
somente o próprio JSON de evidência e remove o scratch antes da segunda
captura. Portanto, `sourceInputsDirty=false` atribui a rodada ao commit indicado
em `gitCommit`; mudança concorrente de `HEAD`, arquivo rastreado ou não
rastreado fecha o gate.

Uma rodada canônica pode ainda receber `-ArtifactDirectory` com um filho direto
e inédito de `.scratch/productive-journey-evidence/`. Nesse modo, o gate retém,
sem versioná-los, a captura do Canvas, o JPEG exportado, a mídia original, os
Projetos salvos, os logs dos testes públicos e os logs JSONL correlacionados dos
processos. `artifact-manifest.json` fixa tamanho e SHA-256 de cada arquivo; o
recibo versionado fixa o SHA-256 desse manifesto. O scratch efêmero da execução
continua removido e o diretório de retenção nunca é sobrescrito.

O estado canônico fica no próprio
`docs/research/artifacts/0023-productive-journey.json`. `gitCommit`,
`sourceInputsDirty`, `proofLayer`, `coverageLimits`, hashes, versões de Windows,
WebView2 e driver e `cleanupCompleted` são a autoridade para atribuir cada
rodada ao snapshot e para limitar as conclusões permitidas.
