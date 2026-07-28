import { useEffect, useMemo, useState } from "react";
import { Button } from "react-aria-components";

import type {
  EditorProjection,
  ExportResult,
  ProjectBridge,
  ProjectIntent,
} from "../domain/project";
import type { GraphicsDiagnostic } from "../platform/graphics";
import { useEditorView } from "../state/editorView";
import { AlbumCanvas } from "./AlbumCanvas";

interface ProjectWorkspaceProps {
  projection: EditorProjection;
  bridge: ProjectBridge;
  graphics: GraphicsDiagnostic;
  onProjectionChange(projection: EditorProjection): void;
}

export function ProjectWorkspace({
  projection,
  bridge,
  graphics,
  onProjectionChange,
}: ProjectWorkspaceProps) {
  const selectedFrameId = useEditorView((state) => state.selectedFrameId);
  const focusedSheetId = useEditorView((state) => state.focusedSheetId);
  const viewport = useEditorView((state) => state.viewport);
  const inspectorTab = useEditorView((state) => state.inspectorTab);
  const selectFrame = useEditorView((state) => state.selectFrame);
  const focusSheet = useEditorView((state) => state.focusSheet);
  const setViewport = useEditorView((state) => state.setViewport);
  const setInspectorTab = useEditorView((state) => state.setInspectorTab);
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [exportResult, setExportResult] = useState<ExportResult | null>(null);
  const [materializedCount, setMaterializedCount] = useState(0);

  const selectedFrame = useMemo(
    () =>
      projection.state.album.sheets
        .flatMap((sheet) => sheet.frames)
        .find((frame) => frame.id === selectedFrameId) ?? null,
    [projection.state.album.sheets, selectedFrameId],
  );

  async function run(
    label: string,
    operation: () => Promise<EditorProjection>,
  ) {
    setBusy(label);
    setMessage(null);
    try {
      onProjectionChange(await operation());
    } catch (error: unknown) {
      setMessage(
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setBusy(null);
    }
  }

  function apply(intent: ProjectIntent) {
    return run("Aplicando alteração", () => bridge.apply(intent));
  }

  async function exportPreview() {
    setBusy("Exportando");
    setMessage(null);
    try {
      const result = await bridge.exportPreview();
      setExportResult(result);
    } catch (error: unknown) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(null);
    }
  }

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey) return;
      if (event.key.toLocaleLowerCase() === "z" && projection.state.canUndo) {
        event.preventDefault();
        void run("Desfazendo", bridge.undo);
      }
      if (event.key.toLocaleLowerCase() === "y" && projection.state.canRedo) {
        event.preventDefault();
        void run("Refazendo", bridge.redo);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  });

  const sheetCount = projection.state.album.sheets.length;
  const photoCount = projection.state.album.sheets.reduce(
    (count, sheet) =>
      count + sheet.frames.filter((frame) => frame.photo).length,
    0,
  );

  return (
    <div className="app-shell">
      <header className="titlebar">
        <div className="brand-mini" aria-label="MyAlbuns">
          <span className="brand-mini-mark" aria-hidden="true">
            M
          </span>
          <span>MyAlbuns</span>
        </div>
        <nav className="app-menu" aria-label="Menu principal">
          <button type="button">Arquivo</button>
          <button type="button">Editar</button>
          <button type="button">Lâmina</button>
          <button type="button">Exibir</button>
          <button type="button">Ferramentas</button>
          <button type="button">Ajuda</button>
        </nav>
        <div className="project-title">
          <span>{projection.state.projectName}</span>
          {projection.state.dirty && (
            <span className="dirty-indicator" aria-label="Alterações pendentes">
              •
            </span>
          )}
        </div>
        <div className="hardware-chip" title={graphics.reason}>
          <span className="hardware-dot" aria-hidden="true" />
          <span>{graphics.renderer}</span>
        </div>
      </header>

      <div className="commandbar">
        <div className="command-group">
          <Button
            className="icon-command"
            aria-label="Desfazer"
            isDisabled={!projection.state.canUndo || Boolean(busy)}
            onPress={() => void run("Desfazendo", bridge.undo)}
          >
            ↶
          </Button>
          <Button
            className="icon-command"
            aria-label="Refazer"
            isDisabled={!projection.state.canRedo || Boolean(busy)}
            onPress={() => void run("Refazendo", bridge.redo)}
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
        <span className="revision-label">
          revisão {projection.state.revision}
        </span>
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
              <strong>{sheetCount} Lâminas no Álbum</strong>
            </div>
            <div className="canvas-help">
              <kbd>Alt</kbd> + arrastar: Pan da Foto
              <span>·</span>
              <kbd>Ctrl</kbd> + roda: Zoom do Canvas
            </div>
          </div>
          <AlbumCanvas
            composition={projection.composition}
            selectedFrameId={selectedFrameId}
            focusedSheetId={focusedSheetId}
            viewport={viewport}
            onSelectFrame={selectFrame}
            onFocusSheet={focusSheet}
            onViewportChange={setViewport}
            onPanCommit={(frameId, deltaX, deltaY) =>
              void apply({
                kind: "panPhoto",
                frameId,
                deltaX,
                deltaY,
              })
            }
            onZoomCommit={(frameId, delta) =>
              void apply({ kind: "zoomPhoto", frameId, delta })
            }
            onMaterializedChange={setMaterializedCount}
          />
          <div className="canvas-status">
            <span>
              {materializedCount}/{sheetCount} Lâminas materializadas
            </span>
            <div className="zoom-controls" aria-label="Zoom do Canvas">
              <button
                type="button"
                aria-label="Reduzir Zoom do Canvas"
                onClick={() =>
                  setViewport({
                    ...viewport,
                    zoom: Math.max(0.45, viewport.zoom - 0.1),
                  })
                }
              >
                −
              </button>
              <span>{Math.round(viewport.zoom * 100)}%</span>
              <button
                type="button"
                aria-label="Aumentar Zoom do Canvas"
                onClick={() =>
                  setViewport({
                    ...viewport,
                    zoom: Math.min(1.55, viewport.zoom + 0.1),
                  })
                }
              >
                +
              </button>
            </div>
          </div>
        </section>

        <aside className="inspector" aria-label="Painel contextual">
          <div className="inspector-tabs">
            <Button
              className={inspectorTab === "album" ? "active" : ""}
              onPress={() => setInspectorTab("album")}
            >
              Geral
            </Button>
            <Button
              className={inspectorTab === "sheets" ? "active" : ""}
              onPress={() => setInspectorTab("sheets")}
            >
              Lâminas
            </Button>
          </div>
          {inspectorTab === "album" ? (
            <div className="inspector-content">
              <div className="context-heading">
                <span>{selectedFrame ? "Frame selecionado" : "Álbum"}</span>
                <h2>
                  {selectedFrame?.photo?.name ?? projection.state.projectName}
                </h2>
              </div>
              {selectedFrame ? (
                <>
                  <PropertyRow
                    label="Frame"
                    value={selectedFrame.id.replace("frame-", "").toUpperCase()}
                  />
                  <PropertyRow
                    label="Zoom da Foto"
                    value={`${Math.round(
                      (selectedFrame.photo?.transform.userZoom ?? 1) * 100,
                    )}%`}
                  />
                  <PropertyRow
                    label="Pan horizontal"
                    value={`${Math.round(
                      (selectedFrame.photo?.transform.panX ?? 0) * 100,
                    )}%`}
                  />
                  <div className="property-actions">
                    <Button
                      onPress={() =>
                        void apply({
                          kind: "zoomPhoto",
                          frameId: selectedFrame.id,
                          delta: -0.1,
                        })
                      }
                      isDisabled={!selectedFrame.photo}
                    >
                      − Zoom
                    </Button>
                    <Button
                      onPress={() =>
                        void apply({
                          kind: "zoomPhoto",
                          frameId: selectedFrame.id,
                          delta: 0.1,
                        })
                      }
                      isDisabled={!selectedFrame.photo}
                    >
                      + Zoom
                    </Button>
                  </div>
                </>
              ) : (
                <>
                  <PropertyRow label="Lâminas" value={String(sheetCount)} />
                  <PropertyRow
                    label="Fotos posicionadas"
                    value={String(photoCount)}
                  />
                  <PropertyRow
                    label="Dimensão"
                    value="60 × 30 cm"
                  />
                  <PropertyRow label="Resolução" value="300 DPI" />
                  <div className="architecture-note">
                    <span className="architecture-icon" aria-hidden="true">
                      ✓
                    </span>
                    <div>
                      <strong>Estado canônico no Rust</strong>
                      <p>
                        Seleção e navegação ficam somente nesta Janela.
                      </p>
                    </div>
                  </div>
                </>
              )}
            </div>
          ) : (
            <div className="sheet-grid">
              {projection.state.album.sheets.map((sheet) => (
                <Button
                  key={sheet.id}
                  className={
                    sheet.id === focusedSheetId ? "sheet-tile active" : "sheet-tile"
                  }
                  onPress={() => {
                    focusSheet(sheet.id);
                    const index = sheet.number - 1;
                    setViewport({
                      ...viewport,
                      offsetX: 42 - index * (600 + 52) * viewport.zoom,
                    });
                  }}
                >
                  <span className="sheet-miniature">
                    <i />
                    <i />
                  </span>
                  <span>{String(sheet.number).padStart(2, "0")}</span>
                </Button>
              ))}
            </div>
          )}
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
            <span className="media-count">
              {projection.state.album.media.length} Fotos vinculadas
            </span>
          </div>
          <div className="media-strip">
            {projection.state.album.media.map((media) => (
              <button
                className="media-card"
                type="button"
                key={media.id}
                onDoubleClick={() =>
                  void apply({
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

function PropertyRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="property-row">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
