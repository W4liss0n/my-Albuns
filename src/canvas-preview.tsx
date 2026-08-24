import React, {
  useCallback,
  useMemo,
  useRef,
  useState,
} from "react";
import ReactDOM from "react-dom/client";

import "./App.css";
import { AlbumCanvas } from "./components/AlbumCanvas";
import type {
  AlbumCanvasMode,
  CanvasMetrics,
  SheetBarMetadata,
} from "./components/albumCanvasContract";
import { createNormalCanvasLayout } from "./components/canvasSheetViewGeometry";
import {
  type CanvasGraphicsDiagnosticProbe,
  CanvasGraphicsDiagnosticProbeProvider,
} from "./components/canvasGraphicsDiagnosticProbeContext";
import {
  useCanvasModeKeyboardShortcuts,
} from "./components/useCanvasModeKeyboardShortcuts";
import type {
  ComposedFrame,
  ComposedSheet,
  CompositionPlan,
} from "./domain/project";
import "./canvas-preview.css";

const previewGraphicsDiagnosticProbe: CanvasGraphicsDiagnosticProbe = () => ({
  supported: true,
  renderer: "visual-preview",
  reason: "Prévia visual de desenvolvimento.",
  limits: {
    maxTextureSizePx: 16_384,
    maxRenderbufferSizePx: 16_384,
    maxTextureImageUnits: 16,
  },
});

const sheets: readonly ComposedSheet[] = [
  createSheet({
    activeSides: "right",
    background: "#2b2823",
    number: 1,
    widthUm: 300_000,
  }),
  createSheet({
    activeSides: "both",
    frames: [
      placeholder("sheet-002-top", 21_000, 21_000, 258_000, 124_500),
      placeholder("sheet-002-left", 21_000, 154_500, 124_500, 124_500),
      placeholder("sheet-002-center", 154_500, 154_500, 124_500, 124_500),
      placeholder("sheet-002-right", 321_000, 21_000, 258_000, 258_000),
    ],
    number: 2,
    widthUm: 600_000,
  }),
  createSheet({
    activeSides: "both",
    background: "#fdfcfa",
    frames: [
      placeholder("sheet-003-left", 24_000, 24_000, 264_000, 252_000),
      placeholder("sheet-003-right", 312_000, 24_000, 264_000, 252_000),
    ],
    number: 3,
    widthUm: 600_000,
  }),
  createSheet({
    activeSides: "both",
    background: "#efeae1",
    frames: [
      placeholder("sheet-004-wide", 24_000, 24_000, 552_000, 252_000),
    ],
    number: 4,
    widthUm: 600_000,
  }),
  createSheet({
    activeSides: "left",
    background: "#fdfcfa",
    frames: [
      placeholder("sheet-005-final", 24_000, 24_000, 252_000, 252_000),
    ],
    number: 5,
    widthUm: 300_000,
  }),
];

const composition: CompositionPlan = {
  frameBorder: { kind: "none" },
  sheets: [...sheets],
};

const previewTechnicalGuides = {
  bleedUm: 3_000,
  safetyUm: 5_000,
} as const;

const sheetBarMetadata: readonly SheetBarMetadata[] = [
  {
    sheetId: "sheet-001",
    pageNumbers: [1],
    layoutLocked: false,
  },
  {
    sheetId: "sheet-002",
    pageNumbers: [2, 3],
    layoutLocked: false,
  },
  {
    sheetId: "sheet-003",
    pageNumbers: [4, 5],
    layoutLocked: false,
  },
  {
    sheetId: "sheet-004",
    pageNumbers: [6, 7],
    layoutLocked: false,
  },
  {
    sheetId: "sheet-005",
    pageNumbers: [8],
    layoutLocked: false,
  },
];

export function CanvasPreview() {
  const acceptanceSurface =
    new URLSearchParams(window.location.search).get("acceptance") === "editor"
      ? "editor"
      : undefined;
  const layout = useMemo(
    () =>
      createNormalCanvasLayout(
        composition.sheets,
        previewTechnicalGuides.bleedUm,
      ),
    [],
  );
  const centeredOnce = useRef(false);
  const [selectedFrameId, setSelectedFrameId] = useState<string | null>(null);
  const [focusedSheetId, setFocusedSheetId] = useState("sheet-002");
  const [centeredSheetId, setCenteredSheetId] = useState("sheet-002");
  const [viewport, setViewport] = useState({ offsetX: 0 });
  const [mode, setMode] = useState<AlbumCanvasMode>(() => {
    const parameters = new URLSearchParams(window.location.search);
    return parameters.get("mode") === "sheet-editing"
      ? {
          kind: "sheet-editing",
          sheetId: parameters.get("sheet") ?? "sheet-002",
        }
      : { kind: "normal" };
  });
  const enterSheetEditing = useCallback((sheetId: string) => {
    setFocusedSheetId(sheetId);
    setMode({ kind: "sheet-editing", sheetId });
  }, []);
  const centerPreviewOnSheet = useCallback(
    (sheetId: string, metrics: CanvasMetrics) => {
      setFocusedSheetId(sheetId);
      setCenteredSheetId(sheetId);
      const offsetX = layout.centeredOffset(
        sheetId,
        metrics.scale,
        metrics.width,
      );
      if (offsetX === null) return;
      setViewport((current) => ({ ...current, offsetX }));
    },
    [layout],
  );
  const exitSheetEditing = useCallback(() => {
    if (mode.kind === "sheet-editing") {
      setFocusedSheetId(mode.sheetId);
      setCenteredSheetId(mode.sheetId);
    }
    setSelectedFrameId(null);
    setMode({ kind: "normal" });
  }, [centerPreviewOnSheet, mode]);
  useCanvasModeKeyboardShortcuts({
    implicitSheetId: centeredSheetId,
    mode,
    onEnterSheetEditing: enterSheetEditing,
    onExitSheetEditing: exitSheetEditing,
  });

  return (
    <main
      className="canvas-preview canvas-section"
      data-acceptance-surface={acceptanceSurface}
      data-canvas-mode={mode.kind}
      data-development-preview="canvas"
      data-editing-sheet={
        mode.kind === "sheet-editing" ? mode.sheetId : undefined
      }
    >
      <AlbumCanvas
        projectId="canvas-visual-preview"
        mode={mode}
        composition={composition}
        sheetBarMetadata={sheetBarMetadata}
        technicalGuides={previewTechnicalGuides}
        continuousCanvasLayout={layout}
        selectedFrameId={selectedFrameId}
        focusedSheetId={focusedSheetId}
        centeredSheetId={centeredSheetId}
        viewport={viewport}
        onSelectFrame={setSelectedFrameId}
        onEditSheet={enterSheetEditing}
        onFocusSheet={setFocusedSheetId}
        onCenteredSheetChange={setCenteredSheetId}
        onViewportChange={setViewport}
        onTransformPreview={() => undefined}
        onTransformCommit={async () => true}
        onCanvasMetricsChange={(metrics) => {
          if (centeredOnce.current) return;
          centeredOnce.current = true;
          centerPreviewOnSheet("sheet-002", metrics);
        }}
      />
    </main>
  );
}

function createSheet({
  activeSides,
  background = "#fdfcfa",
  frames = [],
  number,
  widthUm,
}: {
  activeSides: ComposedSheet["activeSides"];
  background?: string;
  frames?: ComposedFrame[];
  number: number;
  widthUm: number;
}): ComposedSheet {
  const drawRect = { x: 0, y: 0, width: widthUm, height: 300_000 };
  return {
    activeSides,
    backgrounds: [{ kind: "color", rgb: background, drawRect }],
    base: { rgb: background, drawRect },
    frames,
    heightUm: 300_000,
    number,
    overlays: [],
    sheetId: `sheet-${String(number).padStart(3, "0")}`,
    widthUm,
  };
}

function placeholder(
  frameId: string,
  x: number,
  y: number,
  width: number,
  height: number,
): ComposedFrame {
  return {
    borderFillRects: [],
    clipRect: { x, y, width, height },
    frameId,
    photo: null,
    zIndex: 0,
  };
}

const root = document.getElementById("root");
if (root) {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <CanvasGraphicsDiagnosticProbeProvider
        probe={previewGraphicsDiagnosticProbe}
      >
        <CanvasPreview />
      </CanvasGraphicsDiagnosticProbeProvider>
    </React.StrictMode>,
  );
}
