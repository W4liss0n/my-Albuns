import { useEffect, useMemo, useRef, useState } from "react";
import { Button } from "react-aria-components";

import type {
  EditorProjection,
  ExportResult,
  ProjectBridge,
  ProjectIntent,
} from "../domain/project";
import { useEditorView } from "../state/editorView";
import { AlbumCanvas } from "./AlbumCanvas";
import { sheetOffsetInCanvasPixels } from "./canvasGeometry";

interface ProjectWorkspaceProps {
  projection: EditorProjection;
  bridge: ProjectBridge;
  onProjectionChange(projection: EditorProjection): void;
}

interface ZoomDraft {
  frameId: string;
  startValue: number;
  value: number;
  committing: boolean;
}

const ignoreMaterializedCount = () => undefined;

function messageFromError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export function ProjectWorkspace({
  projection,
  bridge,
  onProjectionChange,
}: ProjectWorkspaceProps) {
  const selectedFrameId = useEditorView((state) => state.selectedFrameId);
  const focusedSheetId = useEditorView((state) => state.focusedSheetId);
  const viewport = useEditorView((state) => state.viewport);
  const selectFrame = useEditorView((state) => state.selectFrame);
  const focusSheet = useEditorView((state) => state.focusSheet);
  const setViewport = useEditorView((state) => state.setViewport);
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [exportResult, setExportResult] = useState<ExportResult | null>(null);
  const [canvasScale, setCanvasScale] = useState(1);
  const [zoomDraft, setZoomDraftState] = useState<ZoomDraft | null>(null);
  const zoomDraftRef = useRef<ZoomDraft | null>(null);

  const selectedFrame = useMemo(
    () =>
      projection.state.album.sheets
        .flatMap((sheet) => sheet.frames)
        .find((frame) => frame.id === selectedFrameId) ?? null,
    [projection.state.album.sheets, selectedFrameId],
  );
  const selectedComposedPhoto = useMemo(
    () =>
      projection.composition.sheets
        .flatMap((sheet) => sheet.frames)
        .find((frame) => frame.frameId === selectedFrameId)?.photo ?? null,
    [projection.composition.sheets, selectedFrameId],
  );

  function setZoomDraft(next: ZoomDraft | null) {
    zoomDraftRef.current = next;
    setZoomDraftState(next);
  }

  async function runWithGlobalFeedback(
    label: string,
    operation: () => Promise<EditorProjection>,
  ) {
    setBusy(label);
    setMessage(null);
    try {
      onProjectionChange(await operation());
    } catch (error: unknown) {
      setMessage(messageFromError(error));
    } finally {
      setBusy(null);
    }
  }

  function applyWithStatus(intent: ProjectIntent) {
    return runWithGlobalFeedback("Aplicando alteração", () =>
      bridge.apply(intent),
    );
  }

  async function commitInteraction(intent: ProjectIntent) {
    setMessage(null);
    try {
      onProjectionChange(await bridge.apply(intent));
    } catch (error: unknown) {
      setMessage(messageFromError(error));
    }
  }

  async function exportPreview() {
    setBusy("Exportando");
    setMessage(null);
    try {
      const result = await bridge.exportPreview();
      setExportResult(result);
    } catch (error: unknown) {
      setMessage(messageFromError(error));
    } finally {
      setBusy(null);
    }
  }

  function beginZoomGesture(frameId: string, currentValue: number) {
    const currentDraft = zoomDraftRef.current;
    if (
      currentDraft?.frameId === frameId &&
      !currentDraft.committing
    ) {
      return;
    }
    setZoomDraft({
      frameId,
      startValue: currentValue,
      value: currentValue,
      committing: false,
    });
  }

  function updateZoomGesture(
    frameId: string,
    currentValue: number,
    nextValue: number,
  ) {
    const currentDraft = zoomDraftRef.current;
    const draft =
      currentDraft?.frameId === frameId && !currentDraft.committing
        ? currentDraft
        : {
            frameId,
            startValue: currentValue,
            value: currentValue,
            committing: false,
          };
    setZoomDraft({
      ...draft,
      value: nextValue,
    });
  }

  async function finishZoomGesture(frameId: string) {
    const draft = zoomDraftRef.current;
    if (!draft || draft.frameId !== frameId || draft.committing) return;

    const delta = Number((draft.value - draft.startValue).toFixed(4));
    if (Math.abs(delta) < 0.0001) {
      setZoomDraft(null);
      return;
    }

    setZoomDraft({ ...draft, committing: true });
    await commitInteraction({
      kind: "zoomPhoto",
      frameId,
      delta,
    });
    if (zoomDraftRef.current?.frameId === frameId) {
      setZoomDraft(null);
    }
  }

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey) return;
      if (event.key.toLocaleLowerCase() === "z" && projection.state.canUndo) {
        event.preventDefault();
        void runWithGlobalFeedback("Desfazendo", bridge.undo);
      }
      if (event.key.toLocaleLowerCase() === "y" && projection.state.canRedo) {
        event.preventDefault();
        void runWithGlobalFeedback("Refazendo", bridge.redo);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  });

  useEffect(() => {
    if (
      zoomDraftRef.current &&
      zoomDraftRef.current.frameId !== selectedFrameId
    ) {
      setZoomDraft(null);
    }
  }, [selectedFrameId]);

  const sheetCount = projection.state.album.sheets.length;
  const photoCount = projection.state.album.sheets.reduce(
    (count, sheet) =>
      count + sheet.frames.filter((frame) => frame.photo).length,
    0,
  );

  const selectedPhotoZoom =
    selectedFrame?.photo?.transform.userZoom ?? 1;
  const displayedPhotoZoom =
    zoomDraft && zoomDraft.frameId === selectedFrame?.id
      ? zoomDraft.value
      : selectedPhotoZoom;

  return (
    <div className="app-shell">
      <header className="titlebar">
        <nav className="app-menu" aria-label="Menu principal">
          <button type="button">Arquivo</button>
          <button type="button">Editar</button>
          <button type="button">Lâmina</button>
          <button type="button">Exibir</button>
          <button type="button">Ferramentas</button>
          <button type="button">Ajuda</button>
        </nav>
      </header>

      <div className="commandbar">
        <div className="command-group">
          <Button
            className="icon-command"
            aria-label="Desfazer"
            isDisabled={!projection.state.canUndo || Boolean(busy)}
            onPress={() =>
              void runWithGlobalFeedback("Desfazendo", bridge.undo)
            }
          >
            ↶
          </Button>
          <Button
            className="icon-command"
            aria-label="Refazer"
            isDisabled={!projection.state.canRedo || Boolean(busy)}
            onPress={() =>
              void runWithGlobalFeedback("Refazendo", bridge.redo)
            }
          >
            ↷
          </Button>
        </div>
        <span className="command-divider" />
        <div className="tool-active">
          <span aria-hidden="true">↖</span>
          <span>Selecionar</span>
        </div>
        <div className="command-spacer" />
        <Button
          className="primary-command"
          onPress={() => void exportPreview()}
          isDisabled={Boolean(busy)}
        >
          <span aria-hidden="true">⇧</span>
          Exportar prova
        </Button>
      </div>

      <div className="workspace-grid">
        <section className="canvas-section" aria-label="Área de composição">
          <div className="canvas-heading">
            <div>
              <span className="canvas-kicker">Canvas contínuo</span>
              <strong>
                {sheetCount} {sheetCount === 1 ? "Lâmina" : "Lâminas"} no Álbum
              </strong>
            </div>
            <div className="canvas-help">
              <kbd>Alt</kbd> + arrastar: Pan da Foto
              <span>·</span>
              <kbd>Alt</kbd> + roda: Zoom da Foto
            </div>
          </div>
          <AlbumCanvas
            composition={projection.composition}
            selectedFrameId={selectedFrameId}
            focusedSheetId={focusedSheetId}
            viewport={viewport}
            photoZoomPreview={
              zoomDraft
                ? {
                    frameId: zoomDraft.frameId,
                    value: zoomDraft.value,
                  }
                : null
            }
            onSelectFrame={selectFrame}
            onFocusSheet={focusSheet}
            onViewportChange={setViewport}
            onPanCommit={(frameId, deltaX, deltaY) =>
              void commitInteraction({
                kind: "panPhoto",
                frameId,
                deltaX,
                deltaY,
              })
            }
            onZoomCommit={(frameId, delta) =>
              void commitInteraction({ kind: "zoomPhoto", frameId, delta })
            }
            onTransformCommit={(
              frameId,
              deltaPanX,
              deltaPanY,
              deltaZoom,
            ) =>
              void commitInteraction({
                kind: "transformPhoto",
                frameId,
                deltaPanX,
                deltaPanY,
                deltaZoom,
              })
            }
            onMaterializedChange={ignoreMaterializedCount}
            onAutoScaleChange={setCanvasScale}
          />
        </section>

        <aside className="inspector" aria-label="Painel contextual">
          <div className="inspector-scroll">
            {selectedFrame ? (
              <>
                <div className="context-heading">
                  <span>Frame selecionado</span>
                  <h2>{selectedFrame.photo?.name ?? "Frame placeholder"}</h2>
                </div>
                <InspectorSection
                  key="frame-photo-design"
                  title="Design"
                  preferenceKey="frame-photo.design"
                  defaultOpen
                >
                  <PropertyRow
                    label="Frame"
                    value={selectedFrame.id
                      .replace("frame-", "")
                      .toUpperCase()}
                  />
                  <PropertyRow
                    label="Pan horizontal"
                    value={`${Math.round(
                      (selectedFrame.photo?.transform.panX ?? 0) * 100,
                    )}%`}
                  />
                  {selectedFrame.photo && selectedComposedPhoto && (
                    <label className="photo-zoom-control">
                      <span className="photo-zoom-label">
                        <span>Zoom da Foto</span>
                        <output>
                          {Math.round(displayedPhotoZoom * 100)}%
                        </output>
                      </span>
                      <input
                        type="range"
                        aria-label="Zoom da Foto"
                        min={
                          selectedComposedPhoto.placement.zoomRange.minimum *
                          100
                        }
                        max={
                          selectedComposedPhoto.placement.zoomRange.maximum *
                          100
                        }
                        step="1"
                        value={Math.round(displayedPhotoZoom * 100)}
                        disabled={Boolean(zoomDraft?.committing)}
                        onPointerDown={() =>
                          beginZoomGesture(
                            selectedFrame.id,
                            selectedPhotoZoom,
                          )
                        }
                        onChange={(event) =>
                          updateZoomGesture(
                            selectedFrame.id,
                            selectedPhotoZoom,
                            Number(event.currentTarget.value) / 100,
                          )
                        }
                        onPointerUp={() =>
                          void finishZoomGesture(selectedFrame.id)
                        }
                        onKeyDown={(event) => {
                          if (
                            [
                              "ArrowLeft",
                              "ArrowRight",
                              "ArrowUp",
                              "ArrowDown",
                              "Home",
                              "End",
                              "PageUp",
                              "PageDown",
                            ].includes(event.key)
                          ) {
                            beginZoomGesture(
                              selectedFrame.id,
                              selectedPhotoZoom,
                            );
                          }
                        }}
                        onKeyUp={(event) => {
                          if (
                            [
                              "ArrowLeft",
                              "ArrowRight",
                              "ArrowUp",
                              "ArrowDown",
                              "Home",
                              "End",
                              "PageUp",
                              "PageDown",
                            ].includes(event.key)
                          ) {
                            void finishZoomGesture(selectedFrame.id);
                          }
                        }}
                        onBlur={() =>
                          void finishZoomGesture(selectedFrame.id)
                        }
                      />
                    </label>
                  )}
                </InspectorSection>
              </>
            ) : (
              <>
                <InspectorSection
                  key="album-information"
                  title="Informações do Álbum"
                  preferenceKey="album.information"
                  defaultOpen
                >
                  <PropertyRow label="Lâminas" value={String(sheetCount)} />
                  <PropertyRow
                    label="Fotos posicionadas"
                    value={String(photoCount)}
                  />
                  <PropertyRow label="Dimensão" value="60 × 30 cm" />
                  <PropertyRow label="Resolução" value="300 DPI" />
                </InspectorSection>
                <InspectorSection
                  key="album-sheet-grid"
                  title="Grade de Lâminas"
                  preferenceKey="album.sheet-grid"
                  defaultOpen
                >
                  <div className="sheet-grid">
                    {projection.state.album.sheets.map((sheet, index) => {
                      const sheetOffset = sheetOffsetInCanvasPixels(
                        projection.composition.sheets,
                        index,
                      );
                      return (
                        <Button
                          key={sheet.id}
                          className={
                            sheet.id === focusedSheetId
                              ? "sheet-tile active"
                              : "sheet-tile"
                          }
                          onPress={() => {
                            focusSheet(sheet.id);
                            setViewport({
                              ...viewport,
                              offsetX: 42 - sheetOffset * canvasScale,
                            });
                          }}
                        >
                          <span className="sheet-miniature">
                            <i />
                            <i />
                          </span>
                          <span>{String(sheet.number).padStart(2, "0")}</span>
                        </Button>
                      );
                    })}
                  </div>
                </InspectorSection>
              </>
            )}
          </div>
        </aside>

        <section className="media-panel" aria-label="Painel de imagens">
          <div className="media-panel-head">
            <div className="media-tabs">
              <button className="active" type="button">
                Fotos
              </button>
              <button type="button">Decorativos</button>
            </div>
            <label className="media-search">
              <span aria-hidden="true">⌕</span>
              <input aria-label="Buscar imagens" placeholder="Buscar imagens" />
            </label>
          </div>
          <div className="media-strip">
            {projection.state.album.media.map((media) => (
              <button
                className="media-card"
                type="button"
                key={media.id}
                onDoubleClick={() =>
                  void applyWithStatus({
                    kind: "fillLeftmostPlaceholder",
                    sheetId: focusedSheetId,
                    mediaId: media.id,
                  })
                }
                title="Duplo clique para preencher o placeholder mais à esquerda da Lâmina em foco"
              >
                <span
                  className="media-thumb"
                  style={{
                    background: `linear-gradient(135deg, ${media.palette[0]}, ${media.palette[1]} 56%, ${media.palette[2]})`,
                  }}
                />
                <span className="media-meta">
                  <strong>{media.name}</strong>
                  <small>{media.usageCount} usos</small>
                </span>
              </button>
            ))}
            <div className="media-tip">
              <kbd>2×</kbd>
              <span>
                Preenche o placeholder mais à esquerda da Lâmina em foco
              </span>
            </div>
          </div>
        </section>
      </div>

      {(busy || message || exportResult) && (
        <div
          className={`operation-toast ${message ? "error" : ""}`}
          role={message ? "alert" : "status"}
        >
          {busy && <span className="toast-spinner" aria-hidden="true" />}
          <div>
            <strong>
              {message
                ? "A operação não foi concluída"
                : busy
                  ? busy
                  : "Exportação concluída"}
            </strong>
            <span>
              {message ??
                (exportResult
                  ? `${exportResult.widthPx} × ${exportResult.heightPx}px · ${exportResult.outputPath}`
                  : "Aguarde…")}
            </span>
          </div>
          {!busy && (
            <button
              type="button"
              aria-label="Fechar mensagem"
              onClick={() => {
                setMessage(null);
                setExportResult(null);
              }}
            >
              ×
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function InspectorSection({
  title,
  preferenceKey,
  defaultOpen = false,
  children,
}: {
  title: string;
  preferenceKey: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(() =>
    readInspectorSectionPreference(preferenceKey, defaultOpen),
  );

  function toggle() {
    setOpen((current) => {
      const next = !current;
      writeInspectorSectionPreference(preferenceKey, next);
      return next;
    });
  }

  return (
    <section className="inspector-section">
      <button
        type="button"
        className="inspector-section-trigger"
        aria-expanded={open}
        onClick={toggle}
      >
        <span>{title}</span>
        <span aria-hidden="true">{open ? "−" : "+"}</span>
      </button>
      {open && <div className="inspector-section-content">{children}</div>}
    </section>
  );
}

function readInspectorSectionPreference(
  preferenceKey: string,
  fallback: boolean,
) {
  try {
    const stored = window.localStorage.getItem(
      `myalbuns.inspector.${preferenceKey}`,
    );
    return stored === null ? fallback : stored === "open";
  } catch {
    return fallback;
  }
}

function writeInspectorSectionPreference(
  preferenceKey: string,
  open: boolean,
) {
  try {
    window.localStorage.setItem(
      `myalbuns.inspector.${preferenceKey}`,
      open ? "open" : "closed",
    );
  } catch {
    // The in-memory preference remains usable when storage is unavailable.
  }
}

function PropertyRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="property-row">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
