# Correção de olhos — implementação e validação

## Contratos e arquitetura

A ação **Abrir olhos** pertence à janela nativa do visualizador. O `ProjectWorkspace` continua dono da sequência de fotos, da demanda ao cache e da apresentação da janela. A janela filha recebe URLs opacas e devolve apenas ações e pontos faciais; ela não abre caminhos do sistema de arquivos. A busca de referência usa todas as fotos do projeto, exceto a imagem a corrigir. A foto original permanece no mesmo lugar.

A detecção usa MediaPipe Tasks Vision **1.0.1**, fixado em `package-lock.json`, com Face Landmarker local em um Web Worker. A configuração permite até oito rostos para escolha explícita e não depende de rede durante o uso. O modelo `face_landmarker.task` tem 3.758.596 bytes e SHA-256 `64184e229b263107bc2b804c6625db1341ff2bb731874b0bcc2fe6544e0bc9ff`. O bundle JS tem 155.465 bytes; as três variantes JS/WASM somam 35.444.140 bytes. O pacote e os componentes BlazeFace/Face Mesh V2 estão sob Apache-2.0; a cópia da licença está em `public/models/LICENSE-APACHE-2.0.txt`. Fontes: [guia Web do Face Landmarker](https://developers.google.com/edge/mediapipe/solutions/vision/face_landmarker/web_js), [cartão do Face Mesh V2](https://storage.googleapis.com/mediapipe-assets/Model%20Card%20MediaPipe%20Face%20Mesh%20V2.pdf) e [cartão do BlazeFace](https://storage.googleapis.com/mediapipe-assets/MediaPipe%20BlazeFace%20Model%20Card%20%28Short%20Range%29.pdf).

O Host valida a sessão e os IDs autorizados. A composição usa os originais na resolução completa, orienta JPEG/TIFF conforme EXIF, limita dimensões e decodificação, alinha cada olho pelos cantos, rejeita pares com abertura/escala/pose inadequadas, adapta a cor da pele e suaviza a borda. O trabalho pesado roda em `spawn_blocking`; a prévia opaca tem até 1.600 px, enquanto o PNG aplicado é produzido diretamente da imagem completa. Esta implementação usa o Host para composição, com os limites e a validação de perfil sRGB da infraestrutura de imagem; mover a operação para o processador de imagens continua sendo uma possível evolução arquitetural.

O resultado é guardado numa pasta `.myalbuns-corrections` ao lado do projeto, fora do cache descartável. **Aplicar** usa a substituição de referência de mídia existente, mantendo a identidade e os ajustes de quadros; o vínculo salvo foi reaberto no visualizador nativo e usado por um teste de exportação com o processador real. A identidade e os ajustes de quadros continuam no fluxo de substituição existente; desfazer/refazer após essa correção ainda não recebeu verificação integrada. Durante a mutação, a fase `applying` bloqueia os controles de cancelamento e navegação. A janela ainda pode ser encerrada pelo sistema operacional; nesse caso, a posse exclusiva nativa protege o derivado durante a mutação. Se ela falhar após o fechamento, o arquivo órfão é descartado.

## Verificações

- `npm test`: 1.460 testes passaram em 146 arquivos.
- `cargo test -p myalbuns-desktop --lib`: 524 passaram, 26 ignorados, nenhum falhou.
- Contratos gerados com o Cargo global e comparados byte a byte: 108 arquivos de domínio e 102 de IPC, sem diferenças com os arquivos versionados. `npm run build` não chegou a essas etapas porque `scripts/Test-Contracts.ps1` exige uma instalação local do Rust ausente neste worktree; `npm run typecheck` e `npx vite build` passaram separadamente. O bundle de produção contém os comandos de correção, o worker, o modelo e WASM locais.
- `npm run typecheck`: passou.
- `npx vitest run src/components/ImageViewer.test.tsx`: 6 testes passaram, incluindo o bloqueio de cancelamento/navegação durante aplicação.
- `cargo test -p myalbuns-desktop --lib applying_correction_keeps_derivative_until_mutation_finishes`: passou, cobrindo posse do PNG durante a mutação e limpeza após falha com janela fechada.
- Teste manual ignorado `eye_correction::qa_tests::renders_real_pair_at_original_resolution_and_exif_orientation`: passou com o par fotográfico Nikki. As saídas original e EXIF 6 têm 864 × 864 px; a ampliação de entrada para 3.456 × 3.456 px gerou saída nessa mesma resolução, comprovando que o resultado aplicado não deriva da prévia de 1.600 px.
- Captura filtrada dos cenários do visualizador: `.scratch/eye-correction/qa/ui-acceptance/report.html` e `evidence.json`. O estado de seleção ilustrado foi identificado como recuperação **sem rostos**. As capturas são evidência manual, com status `captured-unreviewed` e origem alterada; não constituem aprovação automática da interface.
- Jornada no WebView nativo com o projeto isolado `.scratch/eye-correction/qa/NativeEye3.myalbuns` e as fotos reais `nikki-closed.jpg` e `nikki-open-a.jpg`: a URL opaca da imagem aceitou `createImageBitmap`; rostos foram detectados e selecionados nos dois painéis; prévia, aplicação e salvamento passaram. O arquivo original foi comparado byte a byte e permaneceu igual. Evidência: `native-browse.png`, `native-select.png` e `native-preview.png` na mesma pasta. O projeto salvo tem revisão 3 e liga a mídia original por ID ao PNG estável em `.myalbuns-corrections`.
- Um novo processo nativo reabriu o projeto salvo e o visualizador mostrou o PNG corrigido com 864 × 864 px; evidência: `.scratch/eye-correction/qa/native-reopen.png`.
- Teste manual ignorado `project_bootstrap::host::tests::windows_paths::corrected_photo_from_saved_native_project_reopens_and_exports`: passou. Em uma cópia descartável do projeto salvo, `ProjectHost` vinculou a mídia corrigida a uma lâmina, reabriu a cópia e congelou uma exportação cujo único arquivo de origem era exatamente o PNG corrigido. O processador real produziu JPEG de 1.701 × 850 px com 849.019 pixels não brancos. Evidência: `.scratch/eye-correction/qa/native-export-corrected.jpg`. A exportação pela janela de diálogo não foi automatizada nesta verificação; o teste exercita o mesmo pipeline de `ProjectHost` e processador.
- Comparação fotográfica local: `.scratch/eye-correction/qa/eye-comparison.jpg`; os olhos estão abertos e alinhados, com discreta borda clara na pálpebra superior de um lado. Os arquivos em `.scratch` são insumos temporários, não entram no produto.

## Limites conhecidos

### Revisão de detecção em 23 de setembro de 2026

A foto de corpo inteiro `IMG_6187.JPG`, fornecida pelo autor, reproduziu a falha:
o detector retornava zero rostos na imagem inteira, inclusive após reduzir para
1.600 px. A foto de controle retornava um. Reduzir o limiar de detecção de 0,55
até 0,3 não recuperou o rosto; analisar uma região menor da mesma foto recuperou.
A causa confirmada neste caso foi o tamanho do rosto em relação ao enquadramento.

O worker mantém a primeira análise e os limiares existentes. Se não encontrar
rostos, analisa nove regiões sobrepostas de metade da largura e altura, com
superfícies de no máximo 800 px no maior lado. Converte os pontos para as
coordenadas da foto completa, elimina rostos repetidos e mantém o limite de oito.
Todo o trabalho permanece no worker. A busca complementar não é executada quando
a primeira análise já encontrou rostos; portanto, não garante descobrir todos os
rostos menores de um grupo em que algum rosto maior já foi detectado.

O recorte usa `drawImage` sobre a orientação exibida. Durante a investigação,
recortar diretamente um `ImageBitmap` com EXIF preservado produziu uma região
diferente da esperada no navegador; rasterizar o recorte preservou o alinhamento.
Contratos consultados: [Face Landmarker Web](https://developers.google.com/edge/mediapipe/solutions/vision/face_landmarker/web_js),
[drawImage](https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/drawImage)
e [OffscreenCanvas](https://developer.mozilla.org/en-US/docs/Web/API/OffscreenCanvas),
além dos tipos instalados de `@mediapipe/tasks-vision` 1.0.1.

Após a correção, a mesma foto retornou um rosto tanto no original orientado de
4.000 × 6.000 px quanto na prévia JPEG de 1.067 × 1.600 px. O teste real no Edge
levou aproximadamente 432 ms na prévia, incluindo a preparação do detector; a
foto de controle continuou retornando um rosto. A reprodução está na área local
`.scratch/face-detection-debug-20260923/`; a fotografia do autor não é versionada.
Testes do worker cobrem a conversão dos pontos, a deduplicação, a busca limitada
sem rostos e a preservação do caminho rápido. Isso corrige a reprodução fornecida;
não representa uma avaliação geral de precisão do modelo.

A correção aceita originais de até **36 megapixels**, até 10.000 px por eixo, com perfil sRGB conhecido. Pares com olhos pouco abertos, rostos pequenos ou poses/escalas excessivamente diferentes são recusados para evitar composições ruins. A adaptação local de cor não resolve diferenças fortes de luz, óculos ou oclusões; nesses casos, escolher outra referência é necessário. A avaliação de naturalidade foi feita com um par fotográfico real e não cobre todas as condições de retrato.
