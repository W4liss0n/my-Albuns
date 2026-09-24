# Correção de olhos — implementação e validação

## Mudança de fluxo em 23 de setembro de 2026

A decisão atual está em `docs/design/0050-visualizador-integrado-de-imagens.md`:
selecionar os dois rostos prepara a prévia automaticamente; salvar exige a
confirmação **Substituir foto original?** e substitui o arquivo de origem.
Cancelar essa confirmação conserva a prévia, sem escrever no arquivo.

### Medições da otimização

Medições locais no Edge, com as mesmas prévias de até 1.600 px. A busca mantém
os 284 passos e as confirmações: apenas deixa de redimensionar a superfície
de análise quando suas dimensões já são as necessárias. O consumidor também
reutiliza análises concluídas ou em andamento para a mesma URL imutável de
prévia, com limite de 12 imagens; falhas não ficam guardadas.

| Etapa | Antes | Depois | Escopo |
| --- | --- | --- | --- |
| Primeira detecção, seis fotos de grupo | 2,76–3,63 s | 1,82–2,59 s | Média de 3,20 para 2,27 s, cerca de 29% menos tempo nesta rodada |
| Preparação da correção, 4.000 × 6.000 px | 1,078 s | 0,608 s | Leitura e geração da prévia/arquivo completo em paralelo |
| Nova consulta de foto já analisada | 2,111 / 2,450 s | Abaixo de 1 ms | Controle e IMG_6187, com resultado ainda no cache da janela |
| Gravação confirmada de JPEG, 4.000 × 6.000 px | Não se aplica | 2,390 s | Codificação e substituição do arquivo; não inclui regenerar as prévias do projeto |

As nove fotos de verificação conservaram as contagens e todos os limites
normalizados das caixas, com diferença máxima zero. O PNG de resolução completa
do par IMG_6187 / IMG_6186 ficou idêntico byte a byte ao compositor anterior
(SHA-256 `9c75ba9f58442e277d88cae3e2059663cad09b073eafb29f395a23767cd59a44`).
São medições de uma amostra local, não uma garantia
de tempo ou de detecção de todos os rostos.

Os testes de gravação usaram arquivos descartáveis: JPEG, PNG e TIFF de 8 bits
por canal mantêm formato e dimensões; a restauração recupera o conteúdo original da cópia; uma
alteração externa desde a prévia impede a substituição. Um JPEG com orientação
EXIF 6 conservou o perfil ICC e foi gravado com os pixels orientados corretamente.
PNG e TIFF com maior profundidade são recusados antes de preparar a prévia e
antes de substituir o arquivo; os testes conferem que seus bytes permanecem
intactos. O encoder TIFF de `image` 0.25.10 preserva ICC, mas não grava EXIF:
não há promessa de conservar as demais tags TIFF. Isso não equivale a validar
todos os metadados possíveis de cada formato.

Três testes no limite público do `ProjectWorkspace` verificam que respostas
antigas de sucesso e falha não alteram a nova correção, que o novo preparo espera
o cancelamento anterior e que fechar a janela descarta o resultado pendente.
O salvamento usa apenas o token da prévia atual. A confirmação e a saída do modo
também têm cenários renderizados próprios no manifesto de aceitação.

Os resultados das seções seguintes registram a implementação anterior, que preservava o
original e vinculava o projeto a uma cópia corrigida. Eles não comprovam a nova
substituição do original; essa validação deve ser registrada separadamente.

## Contratos e arquitetura

A ação **Abrir olhos** pertence à janela nativa do visualizador. O `ProjectWorkspace` continua dono da sequência de fotos, da demanda ao cache e da apresentação da janela. A janela filha recebe URLs opacas e devolve apenas ações e pontos faciais; ela não abre caminhos do sistema de arquivos. A busca de referência usa todas as fotos do projeto, exceto a imagem a corrigir. A foto original permanece no mesmo lugar.

A detecção usa MediaPipe Tasks Vision **1.0.1**, fixado em `package-lock.json`, com Face Landmarker local em um Web Worker. O modelo permite oito rostos **por análise de região**; a combinação das regiões não trunca a foto a oito pessoas. O uso não depende de rede. O modelo `face_landmarker.task` tem 3.758.596 bytes e SHA-256 `64184e229b263107bc2b804c6625db1341ff2bb731874b0bcc2fe6544e0bc9ff`. O bundle JS tem 155.465 bytes; as três variantes JS/WASM somam 35.444.140 bytes. O pacote e os componentes BlazeFace/Face Mesh V2 estão sob Apache-2.0; a cópia da licença está em `public/models/LICENSE-APACHE-2.0.txt`. Fontes: [guia Web do Face Landmarker](https://developers.google.com/edge/mediapipe/solutions/vision/face_landmarker/web_js), [cartão do Face Mesh V2](https://storage.googleapis.com/mediapipe-assets/Model%20Card%20MediaPipe%20Face%20Mesh%20V2.pdf) e [cartão do BlazeFace](https://storage.googleapis.com/mediapipe-assets/MediaPipe%20BlazeFace%20Model%20Card%20%28Short%20Range%29.pdf).

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
- Teste manual ignorado `project_bootstrap::host::tests::eye_correction_qa::corrected_photo_from_saved_native_project_reopens_and_exports`: passou. Em uma cópia descartável do projeto salvo, `ProjectHost` vinculou a mídia corrigida a uma lâmina, reabriu a cópia e congelou uma exportação cujo único arquivo de origem era exatamente o PNG corrigido. O processador real produziu JPEG de 1.701 × 850 px com 849.019 pixels não brancos. Evidência: `.scratch/eye-correction/qa/native-export-corrected.jpg`. A exportação pela janela de diálogo não foi automatizada nesta verificação; o teste exercita o mesmo pipeline de `ProjectHost` e processador.
- Comparação fotográfica local: `.scratch/eye-correction/qa/eye-comparison.jpg`; os olhos estão abertos e alinhados, com discreta borda clara na pálpebra superior de um lado. Os arquivos em `.scratch` são insumos temporários, não entram no produto.

## Limites conhecidos

### Revisão de detecção em 23 de setembro de 2026

A foto de corpo inteiro `IMG_6187.JPG`, fornecida pelo autor, reproduziu a falha:
o detector retornava zero rostos na imagem inteira, inclusive após reduzir para
1.600 px. A foto de controle retornava um. Reduzir o limiar de detecção de 0,55
até 0,3 não recuperou o rosto; analisar uma região menor da mesma foto recuperou.
A causa confirmada neste caso foi o tamanho do rosto em relação ao enquadramento.

A primeira correção buscava nove regiões apenas quando a foto inteira não
retornava rostos. A revisão com fotos de grupo demonstrou que esse caminho
interrompia a descoberta dos demais rostos e que metade do enquadramento ainda
era grande demais em vários casos. Essa política foi substituída: o worker
mantém a primeira análise e os limiares de 0,55 e sempre complementa a busca em
regiões de metade, um quarto e um oitavo da largura e altura, com sobreposição
de 50%. São 284 análises de descoberta, usando uma superfície reutilizada de
no máximo 800 px no maior lado, seguidas de até duas confirmações por candidato.

Os pontos voltam às coordenadas da foto completa, incluindo a escala de
profundidade. Rostos repetidos são combinados, priorizando pontos afastados
das bordas do recorte. Cada candidato precisa ser encontrado novamente em
um recorte com contexto de três vezes seu tamanho. Candidatos encontrados
apenas nos recortes de um oitavo também precisam passar por um recorte de duas
vezes seu tamanho: uma única confirmação ainda aceitava padrões do piso e
do cenário como rostos em `IMG_6186` e `IMG_6187`. Exigir esse recorte mais
fechado de todos os candidatos, por outro lado, removia um rosto parcialmente
encoberto de `IMG_6276` que já era encontrado nas regiões maiores. A saída usa
os pontos da confirmação com contexto, em vez dos pontos iniciais que podiam
representar apenas a parte inferior desse rosto junto à borda de um recorte.
Todo o processamento continua no worker; não há novo aviso dentro da imagem.

O recorte usa `drawImage` sobre a orientação exibida. Durante a investigação,
recortar diretamente um `ImageBitmap` com EXIF preservado produziu uma região
diferente da esperada no navegador; rasterizar o recorte preservou o alinhamento.
Contratos consultados: [Face Landmarker Web](https://developers.google.com/edge/mediapipe/solutions/vision/face_landmarker/web_js),
[drawImage](https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/drawImage)
e [OffscreenCanvas](https://developer.mozilla.org/en-US/docs/Web/API/OffscreenCanvas),
além dos tipos instalados de `@mediapipe/tasks-vision` 1.0.1.

Validação com o worker de produção no Edge, em prévias JPEG de até 1.600 px:

| Foto | Antes desta revisão | Depois | Conferência visual |
| --- | ---: | ---: | --- |
| IMG_6246 | 1 | 4 | Quatro pessoas, sem marcação no cenário |
| IMG_6252 | 3 | 6 | Seis pessoas |
| IMG_6276 | 7 | 7 | Sete pessoas, incluindo rosto parcialmente encoberto |
| IMG_6300 | 3 | 4 | Quatro pessoas |
| IMG_6510 | 4 | 5 | Cinco pessoas, sem marcação no piso |
| IMG_6498 | 0 | 9 | Melhora parcial; ainda há rostos sem detecção na turma distante |
| IMG_6187 / IMG_6186 | 1 / 1 | 1 / 1 | Nenhuma marcação adicional no piso ou cenário |
| Controle Nikki | 1 | 1 | Rosto preservado |

A análise mais completa custa aproximadamente 2–3 segundos por foto nesta
máquina, em vez dos cerca de 0,1–0,4 segundo do caminho anterior. É uma troca
explícita por cobertura maior; o worker evita bloquear a interface. Fotos
distantes, escuras, de perfil ou com oclusão continuam limitadas pelo modelo
e pela resolução da prévia. Não há garantia de localizar todos os rostos.

`node --test scripts/Test-FaceLandmarksWorker.mjs` cobre busca com resultado
inicial parcial, duas escalas de rostos pequenos, mais de oito rostos,
coordenadas, deduplicação, confirmação de candidatos e liberação do bitmap
em sucesso e falha. Os 12 testes passaram. Os 20 testes do visualizador e
adaptador também passaram; a espera de um teste foi corrigida para observar
a propagação assíncrona do estado da imagem ao botão antes de verificar o bloqueio.
A reprodução local e as fotografias ficam em
`.scratch/face-detection-debug-20260923/`, fora do versionamento. Essa amostra
não constitui uma avaliação geral de precisão do modelo nem uma nova validação
visual de todos os cenários da interface.

### Recusa do par IMG_6187 / IMG_6186

A reprodução com `IMG_6187.JPG` como destino e `IMG_6186.JPG` como referência
encontra um rosto em cada prévia de 1.600 px. O processamento real dos originais
recusava o par com `Os olhos da referência precisam estar visivelmente mais abertos.`.
As aberturas normalizadas medidas foram 0,310 / 0,291 no destino e 0,372 / 0,338
na referência: aumento de aproximadamente 20% / 16%, abaixo dos 35% exigidos
pela regra existente para cada olho. Os olhos estão abertos nas duas fotografias.
A decisão anterior de preservar essa regra foi revista após a nova solicitação
do autor: ela também impedia corrigir uma piscada quando um dos olhos do destino
já estava aberto. A comparação relativa de 35% foi removida. Continua exigida
abertura mínima absoluta de 0,12 nos dois olhos da referência, além dos limites
de tamanho, escala, pose e perfil de cor. A mensagem agora é
`Os olhos da referência precisam estar abertos.`.

Havia também uma falha de apresentação: o Rust devolve erros serializados como
texto, mas o consumidor do visualizador só aproveitava mensagens de `Error`.
O adaptador de correção agora converte esse texto em `Error`, preservando o motivo
no tooltip da foto; falhas desconhecidas mantêm uma mensagem simples da ação.
Contrato consultado para `@tauri-apps/api` 2.11.1:
[tratamento de erros dos comandos Tauri 2](https://v2.tauri.app/develop/calling-rust/#error-handling).
Os quatro testes de `tauriImageViewerWindow.test.ts` cobrem a mensagem ao preparar
e aplicar, a resposta desconhecida e o encaminhamento dos pontos e da prévia.

A reprodução local está em `.scratch/face-detection-debug-20260923/render-pair.mjs`
e `render/Cargo.toml`, que inclui o compositor real. As cópias de diagnóstico e
os pontos das fotos não são versionados. O compositor real passou a gerar prévia
e PNG de 4.000 × 6.000 px para esse par, sem sobrescrever as fotos originais.
Quatro testes Rust verificam referência aberta com abertura semelhante,
destino com um olho já aberto, referência fechada e escala/pose incompatíveis.
Todos passaram. Gerar a composição comprova o funcionamento do fluxo; não
garante naturalidade para qualquer par escolhido pelo usuário.

A correção aceita originais de até **36 megapixels**, até 10.000 px por eixo, com perfil sRGB conhecido. Pares com olhos pouco abertos, rostos pequenos ou poses/escalas excessivamente diferentes são recusados para evitar composições ruins. A adaptação local de cor não resolve diferenças fortes de luz, óculos ou oclusões; nesses casos, escolher outra referência é necessário. A avaliação de naturalidade foi feita com um par fotográfico real e não cobre todas as condições de retrato.
