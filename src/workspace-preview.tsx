import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import type {
  GraphicsDiagnostic,
  GraphicsProbe,
} from "./application/graphics";
import { silentLogger } from "./application/logging";
import type {
  ExportPipelinePort,
  MediaPreviewPort,
  ProjectCorePort,
  ProjectStartupPort,
  ProjectWindowPort,
} from "./application/projectPorts";
import type { ProjectDialogPort } from "./application/projectDialogPort";
import {
  applyWorkspacePreferenceChange,
  createWorkspacePreferences,
  type WorkspacePreferencesPort,
} from "./application/workspacePreferences";
import type { CanvasGraphicsDiagnosticProbe } from "./components/canvasGraphicsDiagnosticProbeContext";
import type {
  EditorProjection,
  ProjectIntent,
  ProjectMutationOutcome,
} from "./domain/project";
import { createTwoSheetProjection } from "./test/projectFixtures";
import { useEditorView } from "./state/editorView";
import groupGeometryCorpus from "../tests/fixtures/frame-group-geometry-cases.json";
import stackCorpus from "../tests/fixtures/frame-stack-cases.json";
import manualFrameCorpus from "../tests/fixtures/manual-frame-cases.json";
import { frameDeletionCorpus } from "./test/frameDeletionPreview";
import { frameContentSwapCorpus } from "./test/frameContentSwapPreview";
import { frameClipboardCorpus } from "./test/frameClipboardPreview";
import { sheetSideSwapCorpus } from "./test/sheetSideSwapPreview";
import { continuousCanvasScale, createCanvasSheetPresentation } from "./components/canvasGeometry";
import { createCanvasSheetViewGeometry, createNormalCanvasLayout } from "./components/canvasSheetViewGeometry";
import "./ui/theme.css";
import "./ui/ui.css";

const previewParameters = new URLSearchParams(window.location.search);
const frameContext = previewParameters.get("frame");
const sideSwapCase = sheetSideSwapCorpus.cases.find((item) => item.name === (previewParameters.get("side-swap") ?? "mixed"))!;
const frameClipboardCase = frameClipboardCorpus.cases.find((item) => item.name === (previewParameters.get("clipboard") ?? "same-group"))!;
const manualFrameCase = manualFrameCorpus.cases.find((item) => item.name === (previewParameters.get("surface") ?? "double"));
const frameDeletionCase = frameDeletionCorpus.cases.find((item) => item.name === (previewParameters.get("deletion") ?? "group"));
const frameSwapCase = frameContentSwapCorpus.cases.find((item) => item.name === (previewParameters.get("swap") ?? "photos"));
const stackCase = stackCorpus.cases.find((item) => item.action === previewParameters.get("stack"));
const decorativeContext = previewParameters.get("decorative");
const previewScale = Number(previewParameters.get("scale") ?? "1");
if ([1, 1.25, 1.5].includes(previewScale)) {
  document.documentElement.style.zoom = String(previewScale);
  document.documentElement.dataset.previewScale = String(previewScale);
}
const structureContext = previewParameters.get("structure");
const unavailableDecorativeId = "decorative-preview-unavailable";
let projection = createPreviewProjection(
  frameContext,
  decorativeContext,
  structureContext,
);
if (frameContext === "multiple" || frameContext === "stack") {
  useEditorView.setState({ projectId: projection.state.projectId, editingSheetId: "sheet-001",
    focusedSheetId: "sheet-001", centeredSheetId: "sheet-001",
    selectedFrameIds: frameContext === "stack" ? stackCase!.selectedFrameIds :
      projection.state.album.sheets[0].frames.map((frame) => frame.id) });
}
if (frameContext === "stack") {
  const exposeSelection = () => { document.body.dataset.stackSelection = useEditorView.getState().selectedFrameIds.join(","); };
  exposeSelection();
  useEditorView.subscribe(exposeSelection);
}
if (frameContext === "manual") {
  const sheetId = manualFrameCase!.sheetId;
  useEditorView.setState({ projectId: projection.state.projectId, editingSheetId: sheetId,
    focusedSheetId: sheetId, centeredSheetId: sheetId, selectedFrameIds: [] });
  exposeManualFrameState();
  useEditorView.subscribe(exposeManualFrameState);
}
const undoStack: EditorProjection[] = [];
const redoStack: EditorProjection[] = [];
let addedSheetSequence = 0;
if (frameContext === "deletion") {
  useEditorView.setState({ projectId: projection.state.projectId, editingSheetId: "sheet-001",
    focusedSheetId: "sheet-001", centeredSheetId: "sheet-001", selectedFrameIds: frameDeletionCase!.selectedFrameIds });
  exposeFrameDeletionState();
  useEditorView.subscribe(exposeFrameDeletionState);
}
if (frameContext === "swap") {
  const normal = previewParameters.get("mode") === "normal";
  const selectedFrameIds = normal ? [] : previewParameters.get("swap") === "empty"
    ? ["swap-frame-2", "swap-frame-3"]
    : frameSwapCase!.selectedFrameIds;
  useEditorView.setState({ projectId: projection.state.projectId, editingSheetId: normal ? null : "sheet-001",
    focusedSheetId: "sheet-001", centeredSheetId: "sheet-001", selectedFrameIds });
  exposeFrameSwapState();
  useEditorView.subscribe(exposeFrameSwapState);
}

if (frameContext === "clipboard") {
  const sheetId = frameClipboardCase.sourceSheetId;
  useEditorView.setState({ projectId: projection.state.projectId, editingSheetId: sheetId,
    focusedSheetId: sheetId, centeredSheetId: sheetId, selectedFrameIds: frameClipboardCase.selectedFrameIds });
  exposeFrameClipboardState();
  useEditorView.subscribe(exposeFrameClipboardState);
}

if (frameContext === "side-swap") {
  const centeredSheetId = previewParameters.get("centered") ?? sideSwapCase.targetSheetId;
  useEditorView.setState({ projectId: projection.state.projectId, editingSheetId: null,
    focusedSheetId: centeredSheetId, centeredSheetId, selectedFrameIds: [] });
  exposeSheetSideSwapState();
  useEditorView.subscribe(exposeSheetSideSwapState);
}

const projectCorePort: ProjectCorePort = {
  load: async () => projection,
  validateAlbumInformation: async () => ({
    errors: [],
    impact: {
      heightPx: 3_543,
      pageWidthPx: 3_543,
      sheetWidthPx: 7_087,
    },
  }),
  apply: async (intent) => applyPreviewIntent(intent).projection,
  applyWithOutcome: async (intent) => applyPreviewIntent(intent),
  importPhoto: async () => ({ kind: "cancelled", projection }),
  readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
  previewFrameGeometry: async () => { throw new Error("Frame geometry preview is not configured in this fixture."); },
  // Replay Core-produced point probes with tolerance for CSS pixel rounding.
  resolvePhotoDropTarget: async (sheetId, xUm, yUm) => frameContext === "swap"
    ? frameContentSwapCorpus.dropProbes.find((probe) => probe.sheetId === sheetId &&
      Math.abs(probe.xUm - xUm) < 6_000 && Math.abs(probe.yUm - yUm) < 6_000)?.target ?? { kind: "invalid" }
    : { kind: "invalid" },
  relink: async () => projection,
  undo: async () => restorePreviewHistory(undoStack, redoStack),
  redo: async () => restorePreviewHistory(redoStack, undoStack),
  save: async () => {
    projection = {
      ...projection,
      state: {
        ...projection.state,
        savedRevision: projection.state.revision,
        dirty: false,
      },
    };
    return {
      outcome: { kind: "saved", revision: projection.state.revision },
      projection,
    };
  },
  saveAs: async () => ({
    outcome: { kind: "cancelled" },
    projection,
  }),
};

const mediaPreviewPort: MediaPreviewPort = {
  prepareMediaPreviews: async () =>
    decorativeContext === "unavailable"
      ? [
          {
            mediaId: unavailableDecorativeId,
            state: "unavailable",
            url: null,
          },
        ]
      : null,
  retryUnavailableMedia: async (mediaId) => ({
    mediaId,
    state: "unavailable",
    url: null,
  }),
  onMediaChanged: async () => () => undefined,
  onCacheProcessorWarning: async () => () => undefined,
};

const exportPipelinePort: ExportPipelinePort = {
  startSheet: () => ({
    completion: Promise.resolve({
      status: "completed",
      result: { heightPx: 3_543, widthPx: 7_087 },
    }),
    cancel: async () => "not_found",
  }),
};

const projectWindowPort: ProjectWindowPort = {
  onCloseRequested: async () => () => undefined,
  requestClose: async () => ({ kind: "closed" }),
  resolveClose: async () => ({ kind: "closed" }),
};

const projectDialogPort: ProjectDialogPort = {
  acquire: () => ({
    dismiss: async () => undefined,
    present: async () => undefined,
  }),
};

const projectStartupPort: ProjectStartupPort = {
  confirmUiReady: async () => undefined,
};

const supportedGraphics: Extract<GraphicsDiagnostic, { supported: true }> = {
  supported: true,
  renderer: "integrated-acceptance-preview",
  reason: "Prévia integrada determinística.",
  limits: {
    maxTextureImageUnits: 16,
    maxRenderbufferSizePx: 16_384,
    maxTextureSizePx: 16_384,
  },
};

const unavailableGraphics: Extract<GraphicsDiagnostic, { supported: false }> = {
  supported: false,
  code: "hardware_unconfirmed",
  renderer: "Microsoft Basic Render Driver",
  reason: "A aceleração WebGL2 por hardware não pôde ser confirmada.",
  limits: null,
};

const graphicsDiagnostic =
  previewParameters.get("graphics") === "unsupported"
    ? unavailableGraphics
    : supportedGraphics;
const graphicsProbe: GraphicsProbe = () => graphicsDiagnostic;

const canvasGraphicsDiagnosticProbe: CanvasGraphicsDiagnosticProbe = () => ({
  ...supportedGraphics,
});

const workspacePreferencesPort = createPreviewWorkspacePreferencesPort(
  previewParameters.get("layout"),
);

const appProps = {
  canvasGraphicsDiagnosticProbe,
  exportPipelinePort,
  graphicsProbe,
  logger: silentLogger,
  mediaPreviewPort,
  projectDialogPort,
  projectCorePort,
  projectStartupPort,
  projectWindowPort,
};

const root = document.getElementById("root");
if (root) {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      {workspacePreferencesPort ? (
        <App {...appProps} workspacePreferencesPort={workspacePreferencesPort} />
      ) : (
        <App {...appProps} workspacePreferencesMode="memory" />
      )}
    </React.StrictMode>,
  );
}

function createPreviewProjection(
  frameMode: string | null,
  decorativeMode: string | null,
  structureMode: string | null,
): EditorProjection {
  if (frameMode === "side-swap") return structuredClone(sideSwapCase.before ?? sheetSideSwapCorpus.before);
  if (frameMode === "clipboard") return structuredClone(frameClipboardCase.before ?? frameClipboardCorpus.before);
  if (frameMode === "swap") return structuredClone(frameContentSwapCorpus.before);
  if (frameMode === "deletion") return structuredClone(frameDeletionCorpus.before);
  if (frameMode === "manual") return structuredClone(manualFrameCase!.before) as EditorProjection;
  const preview = structuredClone(createTwoSheetProjection());
  if (structureMode === "physical") configurePhysicalPreview(preview, 5);
  if (structureMode === "minimum-single-edges") {
    configurePhysicalPreview(preview, 2);
  }
  if (decorativeMode === "unavailable") {
    preview.state.album.media.push({
      id: unavailableDecorativeId,
      kind: "decorative",
      name: "Overlay indisponível.png",
      sourceWidthPx: 2_400,
      sourceHeightPx: 1_800,
      palette: ["#17344a", "#88b7c5", "#d4a15e"],
    });
    preview.state.album.visualDefaults.overlay = {
      scope: "bothSides",
      both: { kind: "media", mediaId: unavailableDecorativeId },
    };
    for (const sheet of preview.composition.sheets) {
      sheet.overlays.push({
        mediaId: unavailableDecorativeId,
        name: "Overlay indisponível.png",
        drawRect: {
          x: 0,
          y: 0,
          width: sheet.widthUm,
          height: sheet.heightUm,
        },
      });
    }
    preview.mediaUsage.push({
      mediaId: unavailableDecorativeId,
      count: preview.composition.sheets.length,
    });
  }
  if (frameMode === "multiple" || frameMode === "stack") {
    const frames = frameMode === "stack" ? stackCorpus.before :
      groupGeometryCorpus.cases.find((item) => item.name === "selected")!.frames;
    setPreviewFrames(preview, frames);
    return preview;
  }
  if (frameMode !== "photo" && frameMode !== "empty") return preview;

  const sheet = preview.state.album.sheets[0];
  const frame = sheet?.frames[0];
  const composedSheet = preview.composition.sheets[0];
  const composedFrame = composedSheet?.frames[0];
  if (!sheet || !frame || !composedSheet || !composedFrame) return preview;

  const fullSheetRect = {
    x: 0,
    y: 0,
    width: sheet.widthUm,
    height: sheet.heightUm,
  };
  frame.rect = fullSheetRect;
  composedFrame.clipRect = fullSheetRect;
  if (frameMode === "empty") {
    frame.photo = null;
    composedFrame.photo = null;
  }
  return preview;
}

function setPreviewFrames(preview: EditorProjection, frames: typeof stackCorpus.before) {
  const original = preview.state.album.sheets[0].frames.find((frame) => frame.photo)!;
  preview.state.album.sheets[0].frames = frames.map((frame) => ({
    ...original, id: frame.frameId, rect: frame.clipRect, zIndex: frame.zIndex,
    photo: frame.photo ? original.photo : null,
  }));
  preview.composition.sheets[0].frames = frames.map((frame) => ({ ...frame,
    photo: frame.photo ? { ...frame.photo,
      palette: [frame.photo.palette[0], frame.photo.palette[1], frame.photo.palette[2]] } : null,
  }));
}

function configurePhysicalPreview(
  preview: EditorProjection,
  sheetCount: number,
) {
  const sheetWidthUm = preview.state.document.sheetWidthUm;
  const sheetHeightUm = preview.state.document.sheetHeightUm;
  const ids = Array.from(
    { length: sheetCount },
    (_, index) => `sheet-${String(index + 1).padStart(3, "0")}`,
  );
  let nextPageNumber = 1;
  preview.state.album.sheets = ids.map((id, index) => {
    const activeSides =
      index === 0 ? "right" : index === ids.length - 1 ? "left" : "both";
    const pageCount = activeSides === "both" ? 2 : 1;
    const pageNumbers = Array.from(
      { length: pageCount },
      () => nextPageNumber++,
    );
    return {
      id,
      number: index + 1,
      role:
        index === 0
          ? "initial"
          : index === ids.length - 1
            ? "final"
            : "internal",
      activeSides,
      pageNumbers,
      widthUm: sheetWidthUm,
      heightUm: sheetHeightUm,
      frames: [],
    };
  });
  preview.composition.sheets = preview.state.album.sheets.map((sheet) =>
    blankPreviewCompositionSheet(preview, sheet.id),
  );
  preview.mediaUsage = preview.state.album.media.map((media) => ({
    mediaId: media.id,
    count: 0,
  }));
}

function applyPreviewIntent(intent: ProjectIntent): ProjectMutationOutcome {
  if (intent.kind === "swapSheetSides") {
    if (frameContext !== "side-swap" || intent.sheetId !== sideSwapCase.targetSheetId || sideSwapCase.outcome === "unavailable") {
      throw new Error("Comando fora do cenário de Troca de lados desta prévia.");
    }
    if (sideSwapCase.outcome === "changed") {
      const before = sideSwapCase.before ?? sheetSideSwapCorpus.before;
      const next = JSON.stringify(projection.state.album) === JSON.stringify(before.state.album) ? sideSwapCase.after : before;
      projection = finalizePhysicalPreviewMutation(structuredClone(next), structuredClone(projection));
    }
    exposeSheetSideSwapState();
    return { projection, affectedFrameId: null, affectedSheetId: null };
  }
  if (intent.kind === "copyFrames") {
    if (frameContext !== "clipboard" || [...intent.frameIds].sort().join() !== [...frameClipboardCase.selectedFrameIds].sort().join()) {
      throw new Error("Seleção fora do cenário de cópia desta prévia.");
    }
    projection = structuredClone(frameClipboardCase.copied ?? frameClipboardCorpus.copied);
    exposeFrameClipboardState();
    return { projection, affectedFrameId: null, affectedSheetId: null };
  }
  if (intent.kind === "pasteFrames") {
    if (frameContext !== "clipboard" || !projection.canPasteFrames || intent.sheetId !== frameClipboardCase.targetSheetId || intent.desiredOffsetUm !== frameClipboardCase.desiredOffsetUm) {
      throw new Error("Destino fora do cenário de colagem desta prévia.");
    }
    projection = finalizePhysicalPreviewMutation(structuredClone(frameClipboardCase.after), structuredClone(projection));
    exposeFrameClipboardState();
    return { projection, affectedFrameId: null, affectedSheetId: null, affectedFrameIds: frameClipboardCase.pastedFrameIds };
  }
  if (frameContext === "swap") document.body.dataset.frameSwapLastIntent = JSON.stringify(intent);
  if (intent.kind === "swapFrameContents") {
    if (frameContext !== "swap" || !frameSwapCase ||
        [...intent.frameIds].sort().join() !== [...frameSwapCase.selectedFrameIds].sort().join() ||
        JSON.stringify(projection.state.album.sheets[0].frames) !==
          JSON.stringify(frameContentSwapCorpus.before.state.album.sheets[0].frames)) {
      throw new Error("Comando fora do cenário de troca de conteúdo desta prévia.");
    }
    const before = structuredClone(projection);
    projection = finalizePhysicalPreviewMutation(structuredClone(frameSwapCase.after), before);
    exposeFrameSwapState();
    return { projection, affectedFrameId: null, affectedSheetId: null };
  }
  if (intent.kind === "deleteFrames") {
    if (frameContext !== "deletion" || !frameDeletionCase ||
        [...intent.frameIds].sort().join() !== [...frameDeletionCase.selectedFrameIds].sort().join() ||
        projection.state.album.sheets[0].frames.map((frame) => frame.id).join() !==
          frameDeletionCorpus.before.state.album.sheets[0].frames.map((frame) => frame.id).join()) {
      throw new Error("Comando fora do cenário de exclusão desta prévia.");
    }
    const before = structuredClone(projection);
    projection = finalizePhysicalPreviewMutation(structuredClone(frameDeletionCase.after), before);
    exposeFrameDeletionState();
    return { projection, affectedFrameId: null, affectedSheetId: null };
  }
  if (intent.kind === "addFrame") {
    const sheet = projection.state.album.sheets.find((item) => item.id === intent.sheetId);
    if (frameContext !== "manual" || !manualFrameCase || intent.sheetId !== manualFrameCase.sheetId || sheet?.frames.length !== 0) {
      throw new Error("Comando fora do cenário de criação manual desta prévia.");
    }
    const before = structuredClone(projection);
    const next = structuredClone(projection);
    next.state.album.sheets.find((item) => item.id === intent.sheetId)!.frames = [structuredClone(manualFrameCase.frame)];
    next.composition.sheets.find((item) => item.sheetId === intent.sheetId)!.frames = [structuredClone(manualFrameCase.composedFrame)];
    projection = finalizePhysicalPreviewMutation(next, before);
    exposeManualFrameState();
    return { projection, affectedFrameId: manualFrameCase.frame.id, affectedSheetId: null };
  }
  if (intent.kind === "arrangeFrames") {
    // This fixture replays Core-produced outcomes; it does not implement stack policy.
    if (!stackCase || intent.action !== stackCase.action ||
      [...intent.frameIds].sort().join() !== [...stackCase.selectedFrameIds].sort().join() ||
      projection.composition.sheets[0].frames.map((frame) => frame.frameId).join() !==
        stackCorpus.before.map((frame) => frame.frameId).join()) {
      throw new Error("Comando fora do cenário de ordenação desta prévia.");
    }
    const before = structuredClone(projection);
    const next = structuredClone(projection);
    setPreviewFrames(next, stackCase.frames);
    projection = finalizePhysicalPreviewMutation(next, before);
    return { projection, affectedFrameId: null, affectedSheetId: null };
  }
  if (
    intent.kind !== "addSheet" &&
    intent.kind !== "deleteSheet" &&
    intent.kind !== "reorderSheet"
  ) {
    return {
      projection,
      affectedFrameId: null,
      affectedSheetId: null,
    };
  }

  const before = structuredClone(projection);
  const next = structuredClone(projection);
  let affectedSheetId: string | null = null;
  if (intent.kind === "addSheet") {
    const anchorIndex = next.state.album.sheets.findIndex(
      (sheet) => sheet.id === intent.anchorSheetId,
    );
    if (anchorIndex < 0) throw new Error("Lâmina não encontrada.");
    const insertionIndex =
      intent.position === "before" ? anchorIndex : anchorIndex + 1;
    const pushedEdge = next.state.album.sheets[anchorIndex];
    const insertsOutside =
      insertionIndex === 0 || insertionIndex === next.state.album.sheets.length;
    if (insertsOutside && pushedEdge?.activeSides !== "both") {
      throw new Error("Uma Página única não pode ser empurrada para o interior.");
    }
    affectedSheetId = `sheet-added-${String(++addedSheetSequence).padStart(3, "0")}`;
    const sheet = {
      id: affectedSheetId,
      number: 0,
      role: "internal" as const,
      activeSides: "both" as const,
      pageNumbers: [] as number[],
      widthUm: next.state.document.sheetWidthUm,
      heightUm: next.state.document.sheetHeightUm,
      frames: [],
    };
    next.state.album.sheets.splice(insertionIndex, 0, sheet);
    next.composition.sheets.splice(
      insertionIndex,
      0,
      blankPreviewCompositionSheet(next, affectedSheetId),
    );
  } else if (intent.kind === "deleteSheet") {
    if (next.state.album.sheets.length <= 2) {
      throw new Error("O Álbum precisa manter ao menos duas Lâminas.");
    }
    const deletedIndex = next.state.album.sheets.findIndex(
      (sheet) => sheet.id === intent.sheetId,
    );
    if (deletedIndex < 0) throw new Error("Lâmina não encontrada.");
    next.state.album.sheets.splice(deletedIndex, 1);
    const compositionIndex = next.composition.sheets.findIndex(
      (sheet) => sheet.sheetId === intent.sheetId,
    );
    if (compositionIndex >= 0) next.composition.sheets.splice(compositionIndex, 1);
    affectedSheetId =
      next.state.album.sheets[
        Math.min(deletedIndex, next.state.album.sheets.length - 1)
      ]?.id ?? null;
  } else {
    const sourceIndex = next.state.album.sheets.findIndex(
      (sheet) => sheet.id === intent.sheetId,
    );
    if (
      sourceIndex < 0 ||
      intent.targetIndex < 0 ||
      intent.targetIndex >= next.state.album.sheets.length
    ) {
      throw new Error("Posição de Lâmina inválida.");
    }
    const [moved] = next.state.album.sheets.splice(sourceIndex, 1);
    next.state.album.sheets.splice(intent.targetIndex, 0, moved);
    if (!physicalSheetOrderIsValid(next)) {
      throw new Error("Uma Página única deve permanecer em sua extremidade.");
    }
    affectedSheetId = intent.sheetId;
  }

  projection = finalizePhysicalPreviewMutation(next, before);
  return { projection, affectedFrameId: null, affectedSheetId };
}

function finalizePhysicalPreviewMutation(
  next: EditorProjection,
  before: EditorProjection,
) {
  const compositionById = new Map(
    next.composition.sheets.map((sheet) => [sheet.sheetId, sheet] as const),
  );
  let nextPageNumber = 1;
  next.state.album.sheets = next.state.album.sheets.map((sheet, index, sheets) => {
    const pageCount = sheet.activeSides === "both" ? 2 : 1;
    const pageNumbers = Array.from(
      { length: pageCount },
      () => nextPageNumber++,
    );
    return {
      ...sheet,
      number: index + 1,
      role:
        index === 0
          ? "initial"
          : index === sheets.length - 1
            ? "final"
            : "internal",
      pageNumbers,
    };
  });
  next.composition.sheets = next.state.album.sheets.flatMap((sheet) => {
    const composed = compositionById.get(sheet.id);
    return composed ? [{ ...composed, number: sheet.number }] : [];
  });
  next.state = {
    ...next.state,
    revision: before.state.revision + 1,
    dirty: true,
    canUndo: true,
    canRedo: false,
  };
  undoStack.push(before);
  redoStack.length = 0;
  return next;
}

function restorePreviewHistory(
  source: EditorProjection[],
  destination: EditorProjection[],
) {
  const restored = source.pop();
  if (!restored) return projection;
  destination.push(structuredClone(projection));
  projection = {
    ...structuredClone(restored),
    state: {
      ...restored.state,
      revision: projection.state.revision + 1,
      dirty: true,
      canUndo: undoStack.length > 0,
      canRedo: redoStack.length > 0,
    },
  };
  if (frameContext === "manual") exposeManualFrameState();
  if (frameContext === "deletion") exposeFrameDeletionState();
  if (frameContext === "swap") exposeFrameSwapState();
  if (frameContext === "clipboard") exposeFrameClipboardState();
  if (frameContext === "side-swap") exposeSheetSideSwapState();
  return projection;
}

function exposeManualFrameState() {
  document.body.dataset.manualFrameSelection = useEditorView.getState().selectedFrameIds.join(",");
  document.body.dataset.manualFrameCount = String(projection.state.album.sheets.reduce((count, sheet) => count + sheet.frames.length, 0));
}

function exposeFrameDeletionState() {
  document.body.dataset.frameDeletionSelection = useEditorView.getState().selectedFrameIds.join(",");
  document.body.dataset.frameDeletionCount = String(projection.state.album.sheets[0].frames.length);
}

function exposeFrameClipboardState() {
  const view = useEditorView.getState();
  document.body.dataset.clipboardSelection = view.selectedFrameIds.join(",");
  document.body.dataset.clipboardCount = String(projection.state.album.sheets.reduce((count, sheet) => count + sheet.frames.length, 0));
  document.body.dataset.clipboardAvailable = String(projection.canPasteFrames);
  document.body.dataset.clipboardEditingSheet = view.editingSheetId ?? "none";
}

function exposeSheetSideSwapState() {
  const view = useEditorView.getState();
  const before = sideSwapCase.before ?? sheetSideSwapCorpus.before;
  document.body.dataset.sideSwapState = JSON.stringify(projection.state.album) === JSON.stringify(before.state.album) ? "before" : "after";
  document.body.dataset.sideSwapHistory = `${projection.state.canUndo},${projection.state.canRedo}`;
  document.body.dataset.sideSwapCentered = view.centeredSheetId ?? "none";
  document.body.dataset.sideSwapEditing = view.editingSheetId ?? "none";
}

function exposeFrameSwapState() {
  document.body.dataset.frameSwapSelection = useEditorView.getState().selectedFrameIds.join(",");
  document.body.dataset.frameSwapPhotos = projection.state.album.sheets[0].frames
    .map((frame) => frame.photo?.mediaId ?? "empty").join(",");
  document.body.dataset.frameSwapAllPhotos = projection.state.album.sheets.flatMap((sheet) => sheet.frames)
    .map((frame) => frame.photo?.mediaId ?? "empty").join(",");
  document.body.dataset.frameSwapRevision = String(projection.state.revision);
}

// Coordinates only: browser gesture tests still dispatch real pointer input to the Canvas.
if (frameContext === "swap" && previewParameters.get("mode") === "normal") {
  Object.assign(window, { normalSwapTest: {
    point(frameId: string) {
      const canvas = document.querySelector(".canvas-host canvas");
      if (!canvas) return null;
      const bounds = canvas.getBoundingClientRect();
      const sheet = projection.composition.sheets.find((item) => item.frames.some((frame) => frame.frameId === frameId));
      const frame = sheet?.frames.find((item) => item.frameId === frameId);
      if (!sheet || !frame) return null;
      const scale = continuousCanvasScale(bounds.height, sheet.heightUm / 1000);
      const layout = createNormalCanvasLayout(projection.composition.sheets, projection.state.document.bleedUm);
      const entry = layout.entriesAtScale(scale).find((item) => item.sheetId === sheet.sheetId)!;
      const presentation = createCanvasSheetPresentation(sheet);
      const view = createCanvasSheetViewGeometry(sheet, presentation, projection.state.document.bleedUm, true);
      return { x: Math.round(bounds.left + useEditorView.getState().viewport.offsetX + scale *
        (entry.left - view.visibleOuterBounds.x + presentation.activeOffsetXPx + (frame.clipRect.x + frame.clipRect.width / 2) / 1000)),
        y: Math.round(bounds.top + 28 + scale * (frame.clipRect.y + frame.clipRect.height / 2) / 1000) };
    },
    viewport: () => useEditorView.getState().viewport.offsetX,
  } });
}

function physicalSheetOrderIsValid(candidate: EditorProjection) {
  const last = candidate.state.album.sheets.length - 1;
  return candidate.state.album.sheets.every((sheet, index) => {
    if (index === 0) return sheet.activeSides === "both" || sheet.activeSides === "right";
    if (index === last) return sheet.activeSides === "both" || sheet.activeSides === "left";
    return sheet.activeSides === "both";
  });
}

function blankPreviewCompositionSheet(
  candidate: EditorProjection,
  sheetId: string,
) {
  const sheet = candidate.state.album.sheets.find((item) => item.id === sheetId);
  if (!sheet) throw new Error("Lâmina de prévia não encontrada.");
  const activeWidthUm =
    sheet.activeSides === "both" ? sheet.widthUm : sheet.widthUm / 2;
  const drawRect = {
    x: 0,
    y: 0,
    width: activeWidthUm,
    height: sheet.heightUm,
  };
  return {
    sheetId,
    number: sheet.number,
    activeSides: sheet.activeSides,
    widthUm: activeWidthUm,
    heightUm: sheet.heightUm,
    base: { rgb: "#FFFFFF", drawRect },
    backgrounds: [{ kind: "color" as const, rgb: "#FFFFFF", drawRect }],
    frames: [],
    overlays: [],
  };
}

function createPreviewWorkspacePreferencesPort(
  layout: string | null,
): WorkspacePreferencesPort | null {
  if (layout !== "persisted" && layout !== "collapsed") return null;
  let preferences = createWorkspacePreferences({
    workspacePanels:
      layout === "persisted"
        ? {
            inspector: { size: 420, visible: true },
            media: { size: 140, visible: true },
          }
        : {
            inspector: { size: 310, visible: false },
            media: { size: 202, visible: false },
          },
  });
  return {
    load: async () => preferences,
    update: async (change) => {
      preferences = applyWorkspacePreferenceChange(preferences, change);
      return preferences;
    },
  };
}
