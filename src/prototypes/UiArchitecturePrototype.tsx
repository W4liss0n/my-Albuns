import {
  useEffect,
  useState,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent,
} from "react";

export type UiArchitecturePrototypeView = "editor" | "map";
export type UiArchitecturePrototypeEditorMode = "edit" | "normal";

type UiArchitecturePrototypeProps = {
  initialEditorMode?: UiArchitecturePrototypeEditorMode;
  initialView?: UiArchitecturePrototypeView;
};

const fitZoom = 1;
const maximumZoom = 4;
const reorderDragThreshold = 5;
const zoomStep = 0.25;

const canonicalSurfaces = [
  {
    availability: "Integrado",
    href: "/welcome-preview.html",
    id: "global.welcome",
    owner: "#13",
    parent: "Aplicativo global",
    title: "Boas-vindas",
  },
  {
    availability: "Integrado",
    href: "/welcome-preview.html",
    id: "global.new-project.configuration",
    owner: "#9",
    parent: "Novo Projeto",
    title: "Configurações",
  },
  {
    availability: "Integrado",
    href: "/welcome-preview.html",
    id: "global.new-project.personalization",
    owner: "#21",
    parent: "Novo Projeto",
    title: "Personalização",
  },
  {
    availability: "Janela nativa do Windows",
    href: "/docs/design/0003-criacao-de-projeto.md",
    id: "native.project-name-location",
    owner: "#13",
    parent: "Sistema operacional",
    title: "Nome e local",
  },
  {
    availability: "Integrado",
    href: "/workspace-preview.html",
    id: "project.normal",
    owner: "#9",
    parent: "Janela do Projeto",
    title: "Modo normal",
  },
  {
    availability: "Protótipo verificável",
    href: "/ui-architecture-prototype.html?view=editor",
    id: "project.edit",
    owner: "#20 e #22",
    parent: "Janela do Projeto",
    title: "Modo de edição",
  },
  {
    availability: "Contrato aceito",
    href: "/docs/design/0004-exportacao-normal.md",
    id: "project.export",
    owner: "#35",
    parent: "Janela do Projeto",
    title: "Exportação",
  },
  {
    availability: "Desabilitado até integração",
    href: "/docs/design/0006-configuracao-da-exportacao-em-lote.md",
    id: "global.batch-export",
    owner: "#39",
    parent: "Aplicativo global",
    title: "Exportação em lote",
  },
  {
    availability: "Somente pelo Projeto",
    href: "/docs/design/0008-configuracao-da-geracao-em-lote.md",
    id: "project.batch-generation",
    owner: "#36",
    parent: "Janela do Projeto",
    title: "Geração em lote",
  },
  {
    availability: "Pertence à tentativa originadora",
    href: "/docs/design/0005-tela-de-problemas.md",
    id: "shared.problems",
    owner: "Owner da tentativa",
    parent: "Superfície pertencente",
    title: "Problemas",
  },
  {
    availability: "Pertence à tentativa originadora",
    href: "/docs/design/0007-progresso-de-operacoes.md",
    id: "shared.progress",
    owner: "Owner da tentativa",
    parent: "Superfície pertencente",
    title: "Progresso",
  },
  {
    availability: "Ausente da Boas-vindas até ligação",
    href: "/docs/design/0009-configuracoes-do-aplicativo.md",
    id: "global.settings",
    owner: "#12, #16 e #23",
    parent: "Aplicativo global",
    title: "Configurações do aplicativo",
  },
] as const;

const prototypeSheets = [
  { id: "sheet-001", label: "Lâmina 01" },
  { id: "sheet-002", label: "Lâmina 02" },
  { id: "sheet-003", label: "Lâmina 03" },
  { id: "sheet-004", label: "Lâmina 04" },
  { id: "sheet-005", label: "Lâmina 05" },
] as const;

type ReorderSurface = "bar" | "grid";

type ReorderGesture = {
  origin: ReorderSurface;
  sourceId: string;
  targetId: string;
};

type ReorderPointer = {
  origin: ReorderSurface;
  pointerId: number;
  sourceId: string;
  startX: number;
  startY: number;
};

type ReorderResult = {
  origin: ReorderSurface;
  state: "cancelled" | "committed" | "invalid";
};

type PrototypeFrame = {
  borderColor: string;
  borderEnabled: boolean;
  height: number;
  id: string;
  kind: "photo" | "placeholder";
  opacity: number;
  width: number;
  x: number;
  y: number;
};

type FrameGesture = {
  deltaHeight: number;
  deltaWidth: number;
  deltaX: number;
  deltaY: number;
  frameId: string;
  kind: "move" | "resize";
};

type ZoomInput =
  | "keyboard-in"
  | "keyboard-out"
  | "reset"
  | "wheel-in"
  | "wheel-out";

type ViewportTransform = {
  offsetX: number;
  offsetY: number;
  zoom: number;
};

const resizeHandles = [
  { id: "nw", label: "canto superior esquerdo" },
  { id: "n", label: "centro superior" },
  { id: "ne", label: "canto superior direito" },
  { id: "e", label: "centro direito" },
  { id: "se", label: "canto inferior direito" },
  { id: "s", label: "centro inferior" },
  { id: "sw", label: "canto inferior esquerdo" },
  { id: "w", label: "centro esquerdo" },
] as const;

const initialFrames: PrototypeFrame[] = [
  {
    borderColor: "#9b6b4b",
    borderEnabled: true,
    height: 34,
    id: "frame-001",
    kind: "photo",
    opacity: 100,
    width: 28,
    x: 8,
    y: 12,
  },
  {
    borderColor: "#527a73",
    borderEnabled: false,
    height: 28,
    id: "frame-002",
    kind: "placeholder",
    opacity: 70,
    width: 24,
    x: 42,
    y: 18,
  },
  {
    borderColor: "#9b6b4b",
    borderEnabled: true,
    height: 30,
    id: "frame-003",
    kind: "photo",
    opacity: 100,
    width: 20,
    x: 70,
    y: 48,
  },
];

function clampZoom(value: number): number {
  return Math.min(maximumZoom, Math.max(fitZoom, value));
}

function moveBefore(order: string[], sourceId: string, targetId: string) {
  const withoutSource = order.filter((id) => id !== sourceId);
  const targetIndex = withoutSource.indexOf(targetId);
  if (targetIndex < 0) return order;
  withoutSource.splice(targetIndex, 0, sourceId);
  return withoutSource;
}

function isValidSheetOrder(order: string[]): boolean {
  const endpointIds = new Set([order[0], order[order.length - 1]]);
  return endpointIds.has("sheet-001") && endpointIds.has("sheet-005");
}

type ReorderStripProps = {
  gesture: ReorderGesture | null;
  onPointerDown: (
    surface: ReorderSurface,
    sheetId: string,
    event: ReactPointerEvent<HTMLElement>,
  ) => void;
  onPointerMove: (
    surface: ReorderSurface,
    sheetId: string,
    event: ReactPointerEvent<HTMLElement>,
  ) => void;
  onPointerUp: (
    surface: ReorderSurface,
    event: ReactPointerEvent<HTMLElement>,
  ) => void;
  order: string[];
  result: ReorderResult | null;
  surface: ReorderSurface;
};

function ReorderStrip({
  gesture,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  order,
  result,
  surface,
}: ReorderStripProps) {
  const isOrigin = gesture?.origin === surface;
  const candidateOrder = isOrigin
    ? moveBefore(order, gesture.sourceId, gesture.targetId)
    : order;
  const validTarget = !isOrigin || isValidSheetOrder(candidateOrder);
  const previewOrder = validTarget ? candidateOrder : order;
  const state = isOrigin
    ? gesture.sourceId === gesture.targetId
      ? "dragging"
      : validTarget
        ? "preview"
        : "invalid-target"
    : result?.origin === surface
      ? result.state
      : "idle";
  const surfaceName = surface === "bar" ? "Barra" : "Grade";

  return (
    <section
      aria-label={`${surfaceName} de Lâminas`}
      className={`prototype-reorder prototype-reorder--${surface}`}
      data-preview-order={previewOrder.join(",")}
      data-reorder-state={state}
      data-reorder-surface={surface}
      data-sheet-order={order.join(",")}
      onPointerUp={(event) => {
        event.stopPropagation();
        onPointerUp(surface, event);
      }}
    >
      <header>
        <strong>{surfaceName}</strong>
        <span>Arraste para reordenar</span>
      </header>
      <div className="prototype-reorder__items">
        {previewOrder.map((sheetId) => {
          const sheet = prototypeSheets.find((item) => item.id === sheetId);
          const isTarget =
            isOrigin &&
            validTarget &&
            gesture.sourceId !== gesture.targetId &&
            gesture.targetId === sheetId;
          return (
            <div className="prototype-reorder__slot" key={sheetId}>
              {isTarget ? (
                <span
                  aria-hidden="true"
                  className="prototype-reorder__placeholder"
                  data-testid="reorder-placeholder"
                />
              ) : null}
              <button
                aria-label={`Reordenar ${sheet?.label ?? sheetId} pela ${surfaceName}`}
                className={
                  gesture?.sourceId === sheetId && isOrigin
                    ? "prototype-reorder__sheet prototype-reorder__sheet--source"
                    : "prototype-reorder__sheet"
                }
                data-drag-handle="true"
                data-reorder-shift={
                  isOrigin && previewOrder.indexOf(sheetId) !== order.indexOf(sheetId)
                    ? "true"
                    : undefined
                }
                data-sheet-id={sheetId}
                onPointerDown={(event) => {
                  if (event.button !== 0) return;
                  onPointerDown(surface, sheetId, event);
                }}
                onPointerMove={(event) => onPointerMove(surface, sheetId, event)}
                type="button"
              >
                <span>{sheet?.label}</span>
                <small>{sheetId === "sheet-001" || sheetId === "sheet-005" ? "1 página" : "2 páginas"}</small>
              </button>
            </div>
          );
        })}
      </div>
      {isOrigin ? (
        <div
          aria-hidden="true"
          className="prototype-reorder__ghost"
          data-testid="reorder-ghost"
        >
          {prototypeSheets.find((sheet) => sheet.id === gesture.sourceId)?.label}
        </div>
      ) : null}
    </section>
  );
}

export function UiArchitecturePrototype({
  initialEditorMode = "edit",
  initialView = "map",
}: UiArchitecturePrototypeProps) {
  const [view, setView] = useState<UiArchitecturePrototypeView>(initialView);
  const [editorMode, setEditorMode] =
    useState<UiArchitecturePrototypeEditorMode>(initialEditorMode);
  const [viewport, setViewport] = useState<ViewportTransform>({
    offsetX: 0,
    offsetY: 0,
    zoom: fitZoom,
  });
  const zoom = viewport.zoom;
  const [lastZoomInput, setLastZoomInput] = useState<ZoomInput | null>(null);
  const [zoomAnchor, setZoomAnchor] = useState<"center" | "cursor">("center");
  const [zoomOrigin, setZoomOrigin] = useState({ x: 50, y: 50 });
  const [sheetOrder, setSheetOrder] = useState<string[]>(() =>
    prototypeSheets.map((sheet) => sheet.id),
  );
  const [reorderGesture, setReorderGesture] =
    useState<ReorderGesture | null>(null);
  const [reorderPointer, setReorderPointer] =
    useState<ReorderPointer | null>(null);
  const [reorderResult, setReorderResult] =
    useState<ReorderResult | null>(null);
  const [centeredSheetId, setCenteredSheetId] = useState("sheet-003");
  const [historyCount, setHistoryCount] = useState(0);
  const [frames, setFrames] = useState(initialFrames);
  const [selectedFrameIds, setSelectedFrameIds] = useState<string[]>([]);
  const [frameGesture, setFrameGesture] = useState<FrameGesture | null>(null);
  const [layoutLocked, setLayoutLocked] = useState(false);
  const [layoutLockFeedback, setLayoutLockFeedback] = useState(false);

  const changeZoom = (
    delta: number,
    input: ZoomInput,
    anchor: "center" | "cursor",
    anchorPoint = { x: 0, y: 0 },
  ) => {
    setViewport((current) => {
      const nextZoom = clampZoom(current.zoom + delta);
      const ratio = nextZoom / current.zoom;
      const point = anchor === "cursor" ? anchorPoint : { x: 0, y: 0 };
      return {
        offsetX: point.x - ratio * (point.x - current.offsetX),
        offsetY: point.y - ratio * (point.y - current.offsetY),
        zoom: nextZoom,
      };
    });
    setLastZoomInput(input);
    setZoomAnchor(anchor);
    if (anchor === "center") setZoomOrigin({ x: 50, y: 50 });
  };

  const handleCanvasKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (!event.ctrlKey) return;
    if (event.key === "+" || event.key === "=") {
      event.preventDefault();
      changeZoom(zoomStep, "keyboard-in", "center");
    } else if (event.key === "-") {
      event.preventDefault();
      changeZoom(-zoomStep, "keyboard-out", "center");
    } else if (event.key === "0") {
      event.preventDefault();
      setViewport({ offsetX: 0, offsetY: 0, zoom: fitZoom });
      setLastZoomInput("reset");
      setZoomAnchor("center");
      setZoomOrigin({ x: 50, y: 50 });
    }
  };

  const handleCanvasWheel = (event: WheelEvent<HTMLElement>) => {
    if (!event.ctrlKey || event.deltaY === 0) return;
    event.preventDefault();
    const bounds = event.currentTarget.getBoundingClientRect();
    let anchorPoint = { x: 0, y: 0 };
    if (bounds.width > 0 && bounds.height > 0) {
      anchorPoint = {
        x: event.clientX - (bounds.left + bounds.width / 2),
        y: event.clientY - (bounds.top + bounds.height / 2),
      };
      setZoomOrigin({
        x: Math.min(
          100,
          Math.max(0, ((event.clientX - bounds.left) / bounds.width) * 100),
        ),
        y: Math.min(
          100,
          Math.max(0, ((event.clientY - bounds.top) / bounds.height) * 100),
        ),
      });
    }
    changeZoom(
      event.deltaY < 0 ? zoomStep : -zoomStep,
      event.deltaY < 0 ? "wheel-in" : "wheel-out",
      "cursor",
      anchorPoint,
    );
  };

  const changeEditorMode = (nextMode: UiArchitecturePrototypeEditorMode) => {
    setEditorMode(nextMode);
    setReorderGesture(null);
    setReorderPointer(null);
    setReorderResult(null);
    setFrameGesture(null);
    setLayoutLockFeedback(false);
    if (nextMode === "normal") {
      setViewport({ offsetX: 0, offsetY: 0, zoom: fitZoom });
      setLastZoomInput(null);
      setZoomAnchor("center");
      setZoomOrigin({ x: 50, y: 50 });
      setLayoutLocked(false);
      setSelectedFrameIds([]);
    }
  };

  const startReorder = (
    origin: ReorderSurface,
    sourceId: string,
    event: ReactPointerEvent<HTMLElement>,
  ) => {
    setReorderPointer({
      origin,
      pointerId: event.pointerId,
      sourceId,
      startX: event.clientX,
      startY: event.clientY,
    });
    setReorderGesture(null);
    setReorderResult(null);
  };

  const previewReorder = (
    origin: ReorderSurface,
    targetId: string,
    event: ReactPointerEvent<HTMLElement>,
  ) => {
    if (!reorderPointer || reorderPointer.origin !== origin) return;
    if (reorderPointer.pointerId !== event.pointerId) return;
    const distance = Math.hypot(
      event.clientX - reorderPointer.startX,
      event.clientY - reorderPointer.startY,
    );
    if (!reorderGesture && distance < reorderDragThreshold) return;
    setReorderGesture((current) =>
      current && current.origin === origin
        ? { ...current, targetId }
        : {
            origin,
            sourceId: reorderPointer.sourceId,
            targetId,
          },
    );
  };

  const finishReorder = (
    origin: ReorderSurface,
    event: ReactPointerEvent<HTMLElement>,
  ) => {
    if (!reorderPointer || reorderPointer.pointerId !== event.pointerId) return;
    if (reorderPointer.origin !== origin) {
      if (reorderGesture) {
        setReorderResult({
          origin: reorderPointer.origin,
          state: "cancelled",
        });
      }
      setReorderGesture(null);
      setReorderPointer(null);
      return;
    }
    if (!reorderGesture || reorderGesture.origin !== origin) {
      setCenteredSheetId(reorderPointer.sourceId);
      setReorderPointer(null);
      return;
    }
    const finalGesture = reorderGesture;
    const nextOrder = moveBefore(
      sheetOrder,
      finalGesture.sourceId,
      finalGesture.targetId,
    );
    if (
      finalGesture.sourceId === finalGesture.targetId ||
      !isValidSheetOrder(nextOrder)
    ) {
      setReorderResult({ origin, state: "invalid" });
    } else {
      setSheetOrder(nextOrder);
      setHistoryCount((current) => current + 1);
      setReorderResult({ origin, state: "committed" });
    }
    setReorderGesture(null);
    setReorderPointer(null);
  };

  useEffect(() => {
    const cancelReorder = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setReorderPointer(null);
      setReorderGesture((current) => {
        if (current) {
          setReorderResult({ origin: current.origin, state: "cancelled" });
        }
        return null;
      });
    };
    document.addEventListener("keydown", cancelReorder);
    return () => document.removeEventListener("keydown", cancelReorder);
  }, []);

  useEffect(() => {
    const cancelExternalDrop = (event: globalThis.PointerEvent) => {
      if (!reorderPointer) return;
      if (reorderPointer.pointerId !== event.pointerId) return;
      if (reorderGesture) {
        setReorderResult({
          origin: reorderGesture.origin,
          state: event.type === "pointercancel" ? "cancelled" : "invalid",
        });
      }
      setReorderGesture(null);
      setReorderPointer(null);
    };
    window.addEventListener("pointercancel", cancelExternalDrop);
    window.addEventListener("pointerup", cancelExternalDrop);
    return () => {
      window.removeEventListener("pointercancel", cancelExternalDrop);
      window.removeEventListener("pointerup", cancelExternalDrop);
    };
  }, [reorderGesture, reorderPointer]);

  const selectedFrames = frames.filter((frame) =>
    selectedFrameIds.includes(frame.id),
  );
  const opacityValues = new Set(selectedFrames.map((frame) => frame.opacity));
  const colorValues = new Set(
    selectedFrames.map((frame) => frame.borderColor),
  );
  const borderValues = new Set(
    selectedFrames.map((frame) => frame.borderEnabled),
  );
  const opacityIsMixed = opacityValues.size > 1;
  const colorIsMixed = colorValues.size > 1;
  const borderIsMixed = borderValues.size > 1;

  const selectFrame = (frameId: string, additive: boolean) => {
    setSelectedFrameIds((current) => {
      if (!additive) return [frameId];
      return current.includes(frameId)
        ? current.filter((id) => id !== frameId)
        : [...current, frameId];
    });
  };

  const applyOpacity = (value: string) => {
    if (!value) return;
    const opacity = Number(value);
    if (!Number.isFinite(opacity)) return;
    setFrames((current) =>
      current.map((frame) =>
        selectedFrameIds.includes(frame.id) ? { ...frame, opacity } : frame,
      ),
    );
    if (opacityIsMixed) setHistoryCount((current) => current + 1);
  };

  const previewFrame = (frame: PrototypeFrame): PrototypeFrame => {
    if (!frameGesture || frameGesture.frameId !== frame.id) return frame;
    return frameGesture.kind === "move"
      ? {
          ...frame,
          x: frame.x + frameGesture.deltaX,
          y: frame.y + frameGesture.deltaY,
        }
      : {
          ...frame,
          height: frame.height + frameGesture.deltaHeight,
          width: frame.width + frameGesture.deltaWidth,
        };
  };

  const startFrameGesture = (
    frameId: string,
    kind: FrameGesture["kind"],
  ) => {
    if (layoutLocked) {
      setLayoutLockFeedback(true);
      return;
    }
    setLayoutLockFeedback(false);
    setFrameGesture({
      deltaHeight: 0,
      deltaWidth: 0,
      deltaX: 0,
      deltaY: 0,
      frameId,
      kind,
    });
  };

  const previewFrameGesture = (event: ReactPointerEvent<HTMLElement>) => {
    if (!frameGesture) return;
    const target = (event.target as HTMLElement).closest<HTMLElement>(
      "[data-frame-gesture-target]",
    );
    if (!target || target.dataset.frameGestureTarget !== frameGesture.kind) {
      return;
    }
    setFrameGesture((current) =>
      current
        ? {
            ...current,
            deltaHeight: Number(target.dataset.deltaHeight ?? 0),
            deltaWidth: Number(target.dataset.deltaWidth ?? 0),
            deltaX: Number(target.dataset.deltaX ?? 0),
            deltaY: Number(target.dataset.deltaY ?? 0),
          }
        : current,
    );
  };

  const finishFrameGesture = (event: ReactPointerEvent<HTMLElement>) => {
    if (!frameGesture) return;
    const target = (event.target as HTMLElement).closest<HTMLElement>(
      "[data-frame-gesture-target]",
    );
    const valid = target?.dataset.frameGestureTarget === frameGesture.kind;
    if (valid) {
      setFrames((current) => current.map(previewFrame));
      setHistoryCount((current) => current + 1);
    }
    setFrameGesture(null);
  };

  return (
    <main
      className="ui-architecture-prototype"
      data-development-preview="ui-architecture"
      data-prototype-surface={view}
    >
      <header className="ui-architecture-prototype__header">
        <div>
          <p>Programa 05 · protótipo canônico</p>
          <h1>Arquitetura e interação do editor</h1>
        </div>
        <nav aria-label="Seções do protótipo">
          <button
            aria-pressed={view === "map"}
            onClick={() => setView("map")}
            type="button"
          >
            Mapa de superfícies
          </button>
          <button
            aria-pressed={view === "editor"}
            onClick={() => setView("editor")}
            type="button"
          >
            Interações do editor
          </button>
        </nav>
      </header>

      {view === "map" ? (
        <section
          aria-label="Mapa de superfícies"
          className="prototype-surface-map"
        >
          <header>
            <p>IDs estáveis · nomes atuais · ownership explícito</p>
            <h2>Mapa canônico de superfícies</h2>
          </header>
          <p
            className="prototype-surface-map__flow"
            data-testid="surface-transition-map"
          >
            Boas-vindas → Configurações → Personalização → Nome e local →
            Projeto · Modo normal ⇄ Modo de edição
          </p>
          <div className="prototype-surface-map__grid">
            {canonicalSurfaces.map((surface) => (
              <article
                className="prototype-surface-map__node"
                data-surface-id={surface.id}
                data-testid="surface-map-node"
                key={surface.id}
              >
                <span>{surface.id}</span>
                <strong>{surface.title}</strong>
                <p>{surface.parent}</p>
                <dl>
                  <div>
                    <dt>Owner:</dt>
                    <dd>{surface.owner}</dd>
                  </div>
                  <div>
                    <dt>Estado:</dt>
                    <dd>{surface.availability}</dd>
                  </div>
                </dl>
                <a href={surface.href}>Abrir fonte navegável</a>
              </article>
            ))}
          </div>
        </section>
      ) : (
        <section
          aria-label={editorMode === "edit" ? "Modo de edição" : "Modo normal"}
          className={`editor-prototype editor-prototype--${editorMode}`}
          data-centered-sheet-id={centeredSheetId}
          data-editor-mode={editorMode}
          data-history-count={String(historyCount)}
          data-layout-locked={String(layoutLocked)}
        >
          <header className="editor-prototype__toolbar">
            <strong>
              {editorMode === "edit"
                ? "Modo de edição · Lâmina 03"
                : "Modo normal · reordenação de Lâminas"}
            </strong>
            <output aria-live="polite" className="prototype-visually-hidden">
              {zoom === fitZoom
                ? "Visualização ajustada à Lâmina"
                : "Visualização ampliada"}
            </output>
            <div className="editor-prototype__mode-actions">
              <button
                aria-pressed={editorMode === "normal"}
                onClick={() => changeEditorMode("normal")}
                type="button"
              >
                Modo normal
              </button>
              <button
                aria-pressed={editorMode === "edit"}
                onClick={() => changeEditorMode("edit")}
                type="button"
              >
                Modo de edição
              </button>
              {editorMode === "edit" ? (
                <button
                  aria-label="Layout travado"
                  aria-pressed={layoutLocked}
                  onClick={() => {
                    setLayoutLocked((current) => !current);
                    setLayoutLockFeedback(false);
                  }}
                  type="button"
                >
                  Layout travado
                </button>
              ) : null}
            </div>
          </header>
          {editorMode === "normal" ? (
            <ReorderStrip
              gesture={reorderGesture}
              onPointerDown={startReorder}
              onPointerMove={previewReorder}
              onPointerUp={finishReorder}
              order={sheetOrder}
              result={reorderResult}
              surface="bar"
            />
          ) : null}
          <section
            aria-label="Canvas do protótipo"
            className="editor-prototype__canvas"
            data-last-zoom-input={lastZoomInput ?? undefined}
            data-zoom-anchor={zoomAnchor}
            data-zoom-cap={String(maximumZoom)}
            data-zoom-level={String(zoom)}
            data-zoom-offset-x={String(Number(viewport.offsetX.toFixed(2)))}
            data-zoom-offset-y={String(Number(viewport.offsetY.toFixed(2)))}
            data-zoom-origin-x={String(Number(zoomOrigin.x.toFixed(2)))}
            data-zoom-origin-y={String(Number(zoomOrigin.y.toFixed(2)))}
            data-zoom-state={
              zoom === fitZoom ? "fit" : zoom === maximumZoom ? "cap" : "raised"
            }
            onKeyDown={editorMode === "edit" ? handleCanvasKeyDown : undefined}
            onWheel={editorMode === "edit" ? handleCanvasWheel : undefined}
            tabIndex={0}
          >
            <div
              className="editor-prototype__sheet"
              data-testid="prototype-editing-sheet"
              style={{
                transform: `translate(${viewport.offsetX}px, ${viewport.offsetY}px) scale(${zoom})`,
                transformOrigin: "50% 50%",
              }}
            >
              <span>Lâmina 03</span>
              <div
                aria-label="Frames da Lâmina"
                className="prototype-frames"
                onPointerMove={previewFrameGesture}
                onPointerUp={finishFrameGesture}
                role="group"
              >
                {frames.map((frame, index) => {
                  const renderedFrame = previewFrame(frame);
                  const selected = selectedFrameIds.includes(frame.id);
                  return (
                    <button
                      aria-label={`Selecionar Frame 0${index + 1}`}
                      aria-pressed={selected}
                      className={`prototype-frame prototype-frame--${frame.kind}`}
                      data-frame-id={frame.id}
                      data-height={String(renderedFrame.height)}
                      data-opacity={String(frame.opacity)}
                      data-width={String(renderedFrame.width)}
                      data-x={String(renderedFrame.x)}
                      data-y={String(renderedFrame.y)}
                      key={frame.id}
                      onClick={(event) => selectFrame(frame.id, event.ctrlKey)}
                      onPointerDown={(event) => {
                        if (event.button !== 0) return;
                        if (!selected && !event.ctrlKey) {
                          setSelectedFrameIds([frame.id]);
                        }
                        if (editorMode === "edit") {
                          startFrameGesture(frame.id, "move");
                        }
                      }}
                      style={{
                        height: `${renderedFrame.height}%`,
                        left: `${renderedFrame.x}%`,
                        opacity: frame.opacity / 100,
                        top: `${renderedFrame.y}%`,
                        width: `${renderedFrame.width}%`,
                      }}
                      type="button"
                    >
                      <span>{frame.kind === "photo" ? `Foto 0${index + 1}` : "Frame vazio"}</span>
                    </button>
                  );
                })}
                {selectedFrameIds.length > 0 ? (
                  <div
                    aria-hidden={
                      selectedFrameIds.length > 1 ? true : undefined
                    }
                    className={`prototype-frame-selection-bounds${selectedFrameIds.length > 1 ? " prototype-frame-selection-bounds--multiple" : ""}`}
                    data-testid={
                      selectedFrameIds.length > 1
                        ? "frame-selection-bounds"
                        : "frame-single-selection-bounds"
                    }
                    style={
                      selectedFrameIds.length === 1
                        ? (() => {
                            const selected = previewFrame(
                              frames.find(
                                (frame) => frame.id === selectedFrameIds[0],
                              ) as PrototypeFrame,
                            );
                            return {
                              height: `${selected.height}%`,
                              left: `${selected.x}%`,
                              top: `${selected.y}%`,
                              width: `${selected.width}%`,
                            };
                          })()
                        : undefined
                    }
                  >
                    {editorMode === "edit" &&
                    selectedFrameIds.length === 1 &&
                    !layoutLocked
                      ? resizeHandles.map((handle) => (
                          <button
                            aria-label={`Redimensionar Frame 0${frames.findIndex((frame) => frame.id === selectedFrameIds[0]) + 1} pelo ${handle.label}`}
                            data-resize-handle={handle.id}
                            data-testid="frame-resize-handle"
                            key={handle.id}
                            onPointerDown={(event) => {
                              event.stopPropagation();
                              if (event.button !== 0) return;
                              startFrameGesture(selectedFrameIds[0], "resize");
                            }}
                            type="button"
                          />
                        ))
                      : null}
                  </div>
                ) : null}
                {editorMode === "edit" ? (
                  <>
                    <span
                      aria-hidden="true"
                      className="prototype-frame-gesture-target prototype-frame-gesture-target--move"
                      data-delta-x="8"
                      data-delta-y="6"
                      data-frame-gesture-target="move"
                      data-testid="frame-move-target"
                    />
                    <span
                      aria-hidden="true"
                      className="prototype-frame-gesture-target prototype-frame-gesture-target--resize"
                      data-delta-height="8"
                      data-delta-width="10"
                      data-frame-gesture-target="resize"
                      data-testid="frame-resize-target"
                    />
                  </>
                ) : null}
              </div>
            </div>
          </section>
          {editorMode === "edit" && layoutLocked ? (
            <p
              className="prototype-layout-lock-feedback"
              data-blocked={String(layoutLockFeedback)}
              data-layout-lock-feedback=""
              data-testid="layout-lock-feedback"
            >
              Layout travado: seleção preservada; mover e redimensionar estão bloqueados.
            </p>
          ) : null}
          <section
            aria-label="Inspector de Frames"
            className="prototype-frame-inspector"
            data-selection-count={String(selectedFrameIds.length)}
          >
            <header>
              <strong>Frame</strong>
              <span>
                {selectedFrameIds.length > 0
                  ? `${selectedFrameIds.length} Frame${selectedFrameIds.length === 1 ? "" : "s"}`
                  : "Nenhuma seleção"}
              </span>
            </header>
            {selectedFrames.length > 0 ? (
              <div className="prototype-frame-inspector__fields">
                <p>
                  {selectedFrames.length} Frame
                  {selectedFrames.length === 1 ? "" : "s"} · {" "}
                  {selectedFrames.filter((frame) => frame.kind === "photo").length} Foto · {" "}
                  {selectedFrames.filter((frame) => frame.kind === "placeholder").length} placeholder
                </p>
                <label>
                  Opacidade
                  <input
                    aria-label="Opacidade dos Frames"
                    data-mixed-value={opacityIsMixed ? "numeric" : undefined}
                    max="100"
                    min="0"
                    onChange={(event) => applyOpacity(event.target.value)}
                    placeholder={opacityIsMixed ? "—" : undefined}
                    type="number"
                    value={
                      opacityIsMixed
                        ? ""
                        : (selectedFrames[0]?.opacity ?? "")
                    }
                  />
                </label>
                <div
                  className="prototype-frame-inspector__color"
                  data-mixed-value={colorIsMixed ? "color" : undefined}
                  data-testid="mixed-color"
                >
                  <span
                    aria-hidden="true"
                    style={{
                      background: colorIsMixed
                        ? "repeating-linear-gradient(135deg, #d9d3ca 0 5px, #f7f4ef 5px 10px)"
                        : selectedFrames[0]?.borderColor,
                    }}
                  />
                  <span>{colorIsMixed ? "Amostra vazia" : "Cor comum"}</span>
                </div>
                <button
                  aria-checked={
                    borderIsMixed
                      ? "mixed"
                      : Boolean(selectedFrames[0]?.borderEnabled)
                  }
                  aria-label="Borda dos Frames"
                  role="checkbox"
                  type="button"
                >
                  {borderIsMixed ? "Múltiplos" : "Borda"}
                </button>
              </div>
            ) : null}
          </section>
          {editorMode === "normal" ? (
            <ReorderStrip
              gesture={reorderGesture}
              onPointerDown={startReorder}
              onPointerMove={previewReorder}
              onPointerUp={finishReorder}
              order={sheetOrder}
              result={reorderResult}
              surface="grid"
            />
          ) : null}
          <output data-testid="prototype-history-count">
            {historyCount}
          </output>
        </section>
      )}
    </main>
  );
}
