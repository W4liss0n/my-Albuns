---
status: current
document: research
date: 2026-09-16
ticket: 22
platform: windows
---

# Verificação de estilos e transformações

A tarefa [Programa 18 — Estilos e transformações](https://github.com/W4liss0n/my-Albuns/issues/22)
reúne controles já implementados e o Zoom das Fotos em seleção múltipla,
completado nesta etapa. O Zoom coletivo é absoluto: Fotos com ampliações
diferentes mostram um campo indeterminado; informar 175% aplica esse percentual
a todas as Fotos selecionadas, preservando os demais ajustes e ignorando
placeholders. Prévia e confirmação usam o mesmo compositor do Core; confirmar
gera uma única ação de Desfazer/Refazer. Layout travado permite esse ajuste.

## Reconciliação da tarefa

O critério antigo do padrão de borda mencionava prévia própria, `Exibir borda`
e atualização imediata. A decisão vigente no
[design da Janela do Projeto](../design/0001-estrutura-da-janela-do-projeto.md)
usa a miniatura de `Padrões visuais`, cor e espessura na mesma linha, espessura
zero para `sem borda` e confirmação com `Aplicar`. Essa confirmação reúne
Background, Overlay, borda padrão e espaçamento em uma operação, sem salvar o
arquivo automaticamente. A tarefa deve refletir essa decisão já aceita.

## Cobertura dos critérios

Os números seguem os 20 critérios da tarefa. Os caminhos partem da raiz do
repositório; esta matriz identifica testes executáveis e não substitui sua execução.

| Critérios | Comportamento | Verificação principal |
| --- | --- | --- |
| 1–2 | Borda interna, padrão do Álbum, rascunho e Aplicar | `crates/myalbuns-core/tests/frame_style.rs`; `src/components/AlbumDesignForm.test.tsx`; `src/components/InspectorPanelStructure.test.tsx` |
| 3–4 | Opacidade local, estilo personalizado completo e retorno à herança atual | `crates/myalbuns-core/tests/frame_style.rs`; `src/components/FrameStyleControls.test.tsx`; `src/components/useProjectEditorController.frameStyle.test.tsx` |
| 5–7 | Giro, espelhamento, Ângulo com décimos e reset, somente preto e branco nos Efeitos | `crates/myalbuns-core/tests/photo_orientation.rs`; `src/components/PhotoAngleControl.test.tsx`; `src/components/InspectorPanelContext.test.tsx` |
| 8–9 | Transformações não destrutivas, Zoom adicional separado e persistência | `crates/myalbuns-core/tests/photo_orientation.rs`; `crates/myalbuns-core/tests/photo_composition_v3.rs`; `src/components/AlbumCanvas.editingNavigation.test.tsx` |
| 10 | Zoom e deslocamento transitórios do Canvas, limites, âncoras e restauração | `src/components/AlbumCanvas.editingNavigation.test.tsx` |
| 11–12 | Pan/Zoom direto com Alt no modo normal, seleção preservada e uma ação por gesto | `src/components/AlbumCanvas.interactions.test.tsx`; `scripts/Run-FrameGestureRendering.mjs` |
| 13–14 | Ordem das transformações, preenchimento sem vazamentos e recorte preservado ao redimensionar | `crates/myalbuns-core/tests/photo_orientation.rs`; `crates/myalbuns-core/tests/frame_geometry.rs`; `crates/myalbuns-imaging/tests/cli.rs`; `crates/myalbuns-imaging/tests/cli/frame_editing.rs` |
| 15–18 | Seleção múltipla, alcance por tipo, estado misto neutro e ajuste absoluto coletivo | `src/components/InspectorPanelContext.test.tsx`; `src/components/useProjectEditorController.photoZoom.test.tsx`; `src/components/useProjectEditorController.photoAngle.test.tsx`; `src/components/useProjectEditorController.frameStyle.test.tsx`; `crates/myalbuns-core/tests/photo_orientation.rs`; `crates/myalbuns-core/tests/frame_style.rs` |
| 19–20 | Histórico, reabertura, combinações de efeitos e JPEG produtivo | `crates/myalbuns-imaging/tests/cli/frame_editing.rs`; `crates/myalbuns-imaging/tests/cli.rs`; `crates/myalbuns-core/tests/photo_orientation.rs`; `crates/myalbuns-core/tests/layout_session.rs` |

O novo teste `batch_photo_zoom_with_styles_and_effects_keeps_jpeg_through_undo_and_reopening`
cria um Projeto real, importa um PNG, aplica borda, opacidade, Giro, Ângulo,
espelhamento, preto e branco, Pan e Zoom. O processador real exporta antes e
depois do Zoom coletivo. Desfazer recupera os pixels anteriores; Refazer,
salvar e reabrir preservam os pixels posteriores. Os bytes do Original
permanecem intactos. Testes específicos do processador verificam separadamente
a ordem das operações, a borda colorida sobre Foto em preto e branco e a
aplicação única da opacidade.

Os testes da fila cobrem comandos consecutivos de Zoom, Salvar e Desfazer com
uma alteração pendente, tanto em sucesso quanto em falha. A seleção é capturada
na ação; mudar a seleção antes da resposta não redireciona a edição. Respostas
atrasadas de prévia não restauram uma seleção anterior.

## Verificação visual

O manifesto `src/test/uiAcceptanceScenarios.json` acrescenta cenários de Zoom
misto, aplicação coletiva, Histórico e entrada inválida. Eles usam projeções e
prévias geradas pelo Core em `tests/fixtures/photo-orientation-cases.json`.
Os cenários existentes de seleção múltipla também devem ser recapturados, pois
o novo controle altera o conteúdo do Painel contextual. A referência vigente
continua a indicada em `docs/references/ui-programa-diagramacao/README.md`;
os resultados e capturas ficam fora do versionamento.

Comandos de verificação:

```powershell
npm run typecheck
npm test
npm run test:rust
npm run quality:rust
npm run contract:check
npm run test:ui-acceptance
# Definir MYALBUNS_UI_SCENARIO_IDS antes de capturar o conjunto afetado.
npm run ui:acceptance -- -OutputPath .tools/styles-transforms-completion/ui
```
