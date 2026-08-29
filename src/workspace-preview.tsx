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
import "./ui/theme.css";
import "./ui/ui.css";

const previewParameters = new URLSearchParams(window.location.search);
const frameContext = previewParameters.get("frame");
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
const undoStack: EditorProjection[] = [];
const redoStack: EditorProjection[] = [];
let addedSheetSequence = 0;

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
  resolvePhotoDropTarget: async () => ({ kind: "invalid" }),
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
  recoveryStatus: async () =>
    previewParameters.get("recovery") === "available"
      ? { kind: "available" }
      : { kind: "none" },
  resolveRecovery: async () => ({ kind: "deferred" }),
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
  return projection;
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
