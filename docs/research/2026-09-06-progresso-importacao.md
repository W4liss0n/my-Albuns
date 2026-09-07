---
status: current
document: research
date: 2026-09-06
platform: windows
---

# Progresso durante a importação de Fotos

Após testar a release da integração, o usuário confirmou o funcionamento dos
fluxos e relatou uma exceção: o diálogo permanecia em zero e saltava para 134
ao finalizar. A preparação nativa só devolvia a resposta final do lote, o
percurso de recuperação descartava o callback de progresso e o Host avançava
o contador depois de todos os lotes e do commit.

O protocolo 19 transmite o avanço assim que cada Original termina sua
validação e preparação. O Host conserva esses eventos, soma os lotes enquanto
estão ativos e evita duplicação quando uma solicitação precisa ser repetida.
Inspeções alternativas permanecem pendentes até o Host concluir seu trabalho.
Problemas de publicação podem ser informados depois, sem avançar novamente.
A decisão está em
[0020 — Importação com decode único e lotes](../design/0020-importacao-com-decode-unico-e-lotes.md#progresso-durante-os-lotes).

## Reprodução e regressão

O teste `native_import_reports_progress_while_a_later_photo_is_still_processing`
usa dois JPEG reais e mantém o segundo dentro do decoder por uma barreira de
teste já existente. Exige receber `1 de 2` antes de liberar o segundo. Antes
da correção, falhou em duas execuções, em aproximadamente 0,2 s, por ausência
do evento intermediário. Depois passou, inclusive na suíte completa.

O teste do contador cobre lotes simultâneos, repetição do progresso após
recuperação, correlação, limites e conclusão posterior de uma inspeção
alternativa e de uma foto já existente. O protocolo confere a contagem final
contra os resultados tipados de cada lote.

O ensaio `photo_import::native_flow_tests::real_import_flow` passou com
42 fotos importadas, 41 prévias, duas rejeições e uma inspeção alternativa no
Host. Exige avanço intermediário enquanto há trabalho nativo ativo, contagem
sem saltos e notificação tardia de Cache sem duplicação de unidade.

Durante a compilação simultânea da release e dos testes, uma execução usou o
Processador de release no lugar do debug: os hashes dos dois executáveis
coincidiam. Esse perfil ignora deliberadamente a raiz temporária de dados do
teste, resultando em Cache indisponível. A execução com o processador debug
correto passou. `Test-Rust.ps1` agora preserva uma cópia própria do Processador
debug antes de compilar os testes do desktop, que podem repor o sidecar
empacotado na pasta de saída.

## Conferência com as 134 fotos

Uma rodada usou as mesmas 134 fontes autorizadas, Projeto e Cache novos e
nenhuma compilação ou outra suíte em execução. Host e Processador estavam em
perfil debug; o ensaio não inclui WebView nem seletor nativo.

Foram recebidas 135 amostras, de zero a 134, sem repetição nem salto. Os tempos
abaixo partem do início da captura e do planejamento da tentativa.

| Contagem | Tempo | Trabalhos nativos ainda ativos |
| --- | ---: | ---: |
| 1 de 134 | 0,382 s | 8 |
| 34 de 134 | 1,430 s | 8 |
| 67 de 134 | 2,442 s | 8 |
| 101 de 134 | 3,512 s | 8 |
| 134 de 134 | 4,687 s | 1 |

Os oito processos concluíram a fase nativa em 4,664 s; o total até o
atendimento das prévias foi 6,680 s. As 134 fotos e prévias foram confirmadas,
sem rejeições nem decode de Originais no Host. Os hashes dos Originais antes e
depois coincidiram. O índice só foi publicado após o commit, o Monitor não
introduziu outra atualização e Undo removeu a importação em uma ação.

A contagem mede preparação. A conclusão da proposta, publicação e entrega das
miniaturas ainda pode manter o diálogo aberto depois de chegar a 100%.
Esses tempos não medem latência da interface nem demonstram aceleração em
relação a outra versão. A leitura inicial dos hashes aquece o cache do Windows.
O relatório bruto local está em
`D:/CodexBuilds/myalbuns-import-progress-fix/progress-134.json`.

## Validação e artefato

- Suíte Rust: 650 execuções aprovadas, 18 ignoradas conforme a seleção do
  script; os ensaios reais de importação e Exportação rodaram explicitamente.
- Formatação e Clippy do workspace e do supervisor de desenvolvimento passaram.
- Os três cenários do teste da interface sobre progresso da importação passaram.
- Contratos, TypeScript, compilação da interface e build Tauri de release
  passaram. A correção não modifica componentes ou estilos da interface.

A nova release fica em
`D:/CodexBuilds/myalbuns-import-progress-fix/target/release/myalbuns-desktop.exe`,
acompanhada de `myalbuns-imaging.exe`. O programa já aberto pelo usuário
continuou na versão anterior; a confirmação visual do avanço no novo
executável permanece para o teste manual.

SHA-256 do desktop:
`7b07472aaad26fffd317c47b1693a6047fbb95c557a6e1c1a4ce364c8c479a61`.

SHA-256 do Processador de release:
`ac7b116aac81206b9ff3d3e4ec9694dd5c1edae44a12fdad7b61438fbd3aab5c`.

Um teste que exigisse eventos durante o trabalho teria detectado o defeito na
integração original. Essa condição agora é verificada no executável real e
na composição do Host, além do teste da apresentação do progresso.
