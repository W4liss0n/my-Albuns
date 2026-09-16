---
status: current
document: research
date: 2026-09-16
ticket: 20
platform: windows
---

# Verificação da edição de Frames e Fotos

A tarefa [Programa 17 — Edição de Frames e Fotos](https://github.com/W4liss0n/my-Albuns/issues/20)
reúne recursos implementados em etapas anteriores. Esta conferência relaciona seus
44 critérios aos testes existentes e acrescenta uma verificação que atravessa a
edição, o Histórico, o arquivo de Projeto e o processador de Exportação JPEG.
Não introduz novos comandos nem altera o visual do editor.

As regras continuam nas fontes normativas da tarefa, especialmente na
[estrutura da Janela do Projeto](../design/0001-estrutura-da-janela-do-projeto.md).
Os números abaixo correspondem à ordem dos critérios da tarefa nesta data.

## Cobertura dos critérios

Os caminhos na tabela partem da raiz do repositório. Cada arquivo citado contém
testes executáveis; a tabela não substitui a execução dessas verificações.

| Critérios | Comportamento | Verificação principal |
| --- | --- | --- |
| 1, 26–27 | Criar Frames vazios e com Foto, centralização e superfície ativa | `crates/myalbuns-core/tests/manual_frame.rs`; `src/components/useProjectEditorController.manualFrame.test.tsx` |
| 2–3 | Trocar lados preservando numeração, ajustes e Travessias centrais; bloquear Página única | `crates/myalbuns-core/tests/sheet_side_swap.rs`; `src/components/useProjectEditorController.sheetSideSwap.test.tsx`; `src/components/AlbumCanvas.rendering.test.tsx` |
| 4–6, 11–12 | Seleção transitória, entrada/saída da edição, Ctrl, sobreposição e Histórico | `src/state/editorView.test.ts`; `src/components/AlbumCanvas.frameGroup.test.tsx`; `src/components/useProjectEditorController.frameDeletion.test.tsx` |
| 7–9 | Selecionar tudo e Caixa de seleção, cancelamento, foco e Layout travado | `src/components/AlbumCanvas.areaSelection.test.tsx`; `src/components/ProjectWorkspace.test.tsx` |
| 10, 13–18 | Limiar, movimento/redimensionamento de grupo, oito alças, modificadores, limites e uma ação por gesto | `crates/myalbuns-core/tests/frame_geometry.rs`; `src/components/AlbumCanvas.frameGeometry.test.tsx`; `src/components/AlbumCanvas.frameGroup.test.tsx`; `src/components/AlbumCanvas.frameHistory.test.tsx` |
| 19–21 | Organizar a Pilha visual, blocos contíguos, extremos, menus e atalhos | `crates/myalbuns-core/tests/frame_stack.rs`; `src/components/useProjectEditorController.frameStack.test.tsx`; `src/components/ProjectWorkspace.test.tsx` |
| 22–24, 39 | Comandos coletivos, exclusão com/sem Layout travado e preservação da geometria | `crates/myalbuns-core/tests/frame_deletion.rs`; `crates/myalbuns-core/tests/layout_session.rs`; `src/components/useProjectEditorController.frameDeletion.test.tsx` |
| 25 | Alinhar/distribuir automaticamente continua fora do escopo | Catálogo de comandos e menus não oferecem esses comandos. Snaps durante um gesto pertencem à decisão posterior registrada em `docs/design/0033-snap-de-frames.md`. |
| 28–34, 40 | Soltura, substituição, placeholder prioritário, seleção do alvo e reinício dos ajustes da Foto | `crates/myalbuns-core/tests/photo_composition_v3.rs`; `crates/myalbuns-core/tests/manual_frame.rs`; `src/components/AlbumCanvas.interactions.test.tsx`; `src/components/ProjectWorkspace.test.tsx` |
| 35–38 | Máscara, Preenchimento, Pan/Zoom com Alt no modo normal e agrupamento do gesto | `src/components/AlbumCanvas.interactions.test.tsx`; `src/components/photoZoomGesture.test.ts`; `crates/myalbuns-core/tests/photo_composition_v3.rs`; `crates/myalbuns-imaging/tests/cli.rs` |
| 41–42 | Limites físicos e ocorrências independentes do mesmo Original | `crates/myalbuns-core/tests/frame_geometry.rs`; `crates/myalbuns-core/tests/photo_composition_v3.rs`; `crates/myalbuns-imaging/tests/cli/frame_editing.rs` |
| 43 | Histórico, salvar/reabrir e mesma Pilha/recorte no JPEG | `crates/myalbuns-imaging/tests/cli/frame_editing.rs`, além dos testes de persistência de geometria, Pilha e exclusão no Core |
| 44 | Zero, um e vários Frames, proporções extremas, sobreposição e placeholder | `crates/myalbuns-core/tests/manual_frame.rs`; `frame_geometry.rs`; `frame_stack.rs`; `frame_deletion.rs`; `photo_composition_v3.rs` no mesmo diretório |

O ghost aceito posteriormente para arrastar uma imagem do Painel mostra a imagem
de origem. Ele não antecipa a composição, a geometria do novo Frame ou um Layout
futuro, mantendo o contrato do critério 31.

## Percurso acrescentado

`edited_frames_keep_crop_and_stack_through_history_save_reopen_and_jpeg` cria um
Projeto pelo Core e preenche dois placeholders com o mesmo PNG. Uma ocorrência
recebe Pan/Zoom; a outra recebe espelhamento. O teste move e redimensiona o grupo,
confere prévia versus resultado e desfaz/refaz cada gesto em uma ação.

A Exportação usa `RenderAlbum` e o executável real do processador, pelo protocolo
usado pela exportação normal. Cores conhecidas em pontos internos verificam o
recorte, a máscara e qual ocorrência aparece por cima. Alterar a Pilha deve mudar
os pixels da sobreposição; desfazer deve restaurar o JPEG anterior. Depois de
refazer, salvar e reabrir, a composição e os pixels devem permanecer iguais. Os
bytes do Original também são conferidos ao final.

As dimensões e cores esperadas são definidas pelo cenário, sem editar diretamente
o documento persistido nem a composição projetada. Assim, a igualdade entre
dois resultados não consegue, sozinha, aprovar uma saída vazia ou sem as edições.

O teste de substituição em `photo_composition_v3.rs` também foi ampliado: uma
ocorrência recebe Pan, Zoom, Giro, Ângulo, espelhamento e preto e branco, além de
Borda e Opacidade próprias. Substituí-la deve zerar todos os ajustes da Foto,
preservar geometria, estilo e Pilha do Frame e não tocar no Frame sobreposto
inferior. Undo/Redo deve restaurar cada estado em uma única ação.

## Reprodução e limites

```powershell
./scripts/Invoke-LocalCargo.ps1 -CargoArguments @('test', '-p', 'myalbuns-imaging', '--test', 'cli', 'frame_editing::', '--', '--test-threads=2')
npm test
npm run test:rust
npm run typecheck
npm run quality:rust
npm run test:frame-gestures -- .tools/frame-photo-editing-completion/gestures
```

O teste do processador confirma o caminho Core → Projeto salvo → protocolo →
JPEG. Os testes do Canvas e do controlador verificam a entrada e a serialização
dos comandos, inclusive comandos adjacentes ao Salvamento e ao Histórico.
O roteiro de gestos executa movimento e redimensionamento no navegador headless.
Essas verificações não equivalem a uma nova sessão manual na janela nativa.

Logs, capturas e resultados desta conferência ficam em
`.tools/frame-photo-editing-completion/`, fora do controle de versão. Capturas
visuais usam somente cenários declarados em `src/test/uiAcceptanceScenarios.json`,
com decisões registradas individualmente; não atualizam a referência visual.
