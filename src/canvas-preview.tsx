import React, { useMemo, useRef, useState } from "react";
import ReactDOM from "react-dom/client";

import "./App.css";
import { AlbumCanvas } from "./components/AlbumCanvas";
import type { SheetBarMetadata } from "./components/albumCanvasContract";
import {
  createContinuousCanvasLayout,
} from "./components/canvasGeometry";
import {
  type CanvasGraphicsDiagnosticProbe,
  CanvasGraphicsDiagnosticProbeProvider,
} from "./components/canvasGraphicsDiagnosticProbeContext";
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

const sheetBarMetadata: readonly SheetBarMetadata[] = [
  { sheetId: "sheet-001", pageNumbers: [1] },
  { sheetId: "sheet-002", pageNumbers: [2, 3] },
  { sheetId: "sheet-003", pageNumbers: [4, 5] },
  { sheetId: "sheet-004", pageNumbers: [6, 7] },
  { sheetId: "sheet-005", pageNumbers: [8] },
];

function CanvasPreview() {
  const layout = useMemo(
    () => createContinuousCanvasLayout(composition.sheets),
    [],
  );
  const centeredOnce = useRef(false);
  const [focusedSheetId, setFocusedSheetId] = useState("sheet-002");
  const [centeredSheetId, setCenteredSheetId] = useState("sheet-002");
  const [viewport, setViewport] = useState({ offsetX: 0 });

  return (
    <main className="canvas-preview canvas-section" data-development-preview="canvas">
      <AlbumCanvas
        projectId="canvas-visual-preview"
        composition={composition}
        sheetBarMetadata={sheetBarMetadata}
        technicalGuides={{ bleedUm: 3_000, safetyUm: 5_000 }}
        continuousCanvasLayout={layout}
        selectedFrameId={null}
        focusedSheetId={focusedSheetId}
        centeredSheetId={centeredSheetId}
        viewport={viewport}
        onSelectFrame={() => undefined}
        onFocusSheet={setFocusedSheetId}
        onCenteredSheetChange={setCenteredSheetId}
        onViewportChange={setViewport}
        onTransformPreview={() => undefined}
        onTransformCommit={async () => true}
        onCanvasMetricsChange={({ width, scale }) => {
          if (centeredOnce.current) return;
          centeredOnce.current = true;
          setViewport({
            offsetX:
              layout.centeredOffset("sheet-002", scale, width) ?? 0,
          });
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

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <CanvasGraphicsDiagnosticProbeProvider
      probe={previewGraphicsDiagnosticProbe}
    >
      <CanvasPreview />
    </CanvasGraphicsDiagnosticProbeProvider>
  </React.StrictMode>,
);
