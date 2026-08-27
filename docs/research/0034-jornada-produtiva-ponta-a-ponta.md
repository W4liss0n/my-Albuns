---
status: current
document: technical-research
ticket: 8-esqueleto-ponta-a-ponta
date: 2026-08-14
updated: 2026-08-14
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

## Reprodutibilidade

O wrapper captura `HEAD` e a árvore inteira antes e depois da execução, exclui
somente o próprio JSON de evidência e remove o scratch antes da segunda
captura. Portanto, `sourceInputsDirty=false` atribui a rodada ao commit indicado
em `gitCommit`; mudança concorrente de `HEAD`, arquivo rastreado ou não
rastreado fecha o gate.

A rodada canônica registrada em
`docs/research/artifacts/0023-productive-journey.json` executou 8/8 checks sobre
o input limpo `8cb8dc3380911e18b3f9ec0e1f50458df10afeb3`, com
`sourceInputsDirty=false`. O artefato foi publicado no commit posterior
`ea0acf6fe78b3aeebb5cbaad62bcfcb551abe66c`, que altera somente o JSON de
evidência. A captura contém 1.271 de 1.271 amostras não brancas, os três PIDs
distintos, Lâmina 02 dupla contra a primeira de página única, amostra RGB
`31/64/95` com delta máximo 1 em relação ao Background salvo e
`cleanupCompleted=true`.
