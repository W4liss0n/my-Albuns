import { emptyLayoutCatalogPort } from "../test/layoutCatalogPorts";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import type { ComponentProps } from "react";
import { beforeEach, expect, test, vi } from "vitest";

import type {
  ExportAttempt,
  ExportOutcome,
  ExportPipelinePort,
  ExportProgressEvent,
  MediaPreviewDemand,
  ProjectCloseResolution,
  ProjectCorePort,
  ProjectWindowPort,
} from "../application/projectPorts";
import type {
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogSession,
} from "../application/projectDialogPort";
import type { SheetStructureIntent } from "../application/sheetStructure";
import {
  ProjectCloseError,
  SaveProjectError,
} from "../application/projectPorts";
import type { GraphicsDiagnostic } from "../application/graphics";
import {
  applyWorkspacePreferenceChange,
  createFallbackWorkspacePreferencesPort,
  createWorkspacePreferences,
  type WorkspacePreferencesPort,
} from "../application/workspacePreferences";
import type {
  EditorProjection,
  PhotoDropTarget,
} from "../domain/project";
import { useEditorView } from "../state/editorView";
import { frameContentSwapCorpus } from "../test/frameContentSwapPreview";
import { layoutPanelCorpus } from "../test/layoutPanelPreview";
import type { ProjectIntent } from "../domain/project";
import {
  createEmptyProjection,
  createThreeSheetProjection,
  createTwoSheetProjection,
  representativeProjection,
} from "../test/projectFixtures";
import type {
  AlbumCanvasMode,
  CanvasMetrics,
  CanvasSheetReorder,
  CanvasTechnicalGuides,
  CanvasPhotoDropPoint,
  PhotoTransformDelta,
  PhotoTransformPreview,
} from "./AlbumCanvas";
import type { ContinuousCanvasLayout } from "./canvasGeometry";
import { ProjectWorkspace as ProjectWorkspaceView } from "./ProjectWorkspace";
import { useProjectMutationRunner } from "./useProjectMutationRunner";


type ExportPort = LegacyExportPort;
type ProjectSessionPort = ProjectCorePort;

const canvasHarness = vi.hoisted(() => ({
  props: null as null | {
    mode: AlbumCanvasMode;
    sheetLayouts?: { onToggle(sheetId: string): void };
    continuousCanvasLayout: ContinuousCanvasLayout;
    focusedSheetId: string | null;
    centeredSheetId: string | null;
    viewport: { offsetX: number };
    mediaPreviewUrls?: Readonly<Record<string, string>>;
    technicalGuides?: CanvasTechnicalGuides;
    sheetReorder?: CanvasSheetReorder;
    onMediaDemandChange?(demand: MediaPreviewDemand): void;
    onCanvasMetricsChange?(metrics: CanvasMetrics): void;
    onCenteredSheetChange?(sheetId: string): void;
    onEditSheet?(sheetId: string): void;
    draggedPhotoId?: string | null;
    onResolvePhotoDropTarget?(
      mediaId: string,
      point: CanvasPhotoDropPoint,
    ): Promise<PhotoDropTarget>;
    onDropPhoto?(
      mediaId: string,
      point: CanvasPhotoDropPoint,
    ): Promise<boolean>;
    onPhotoDragCancel?(): void;
    onOpenSheetContextMenu?(
      sheetId: string,
      position: { x: number; y: number },
    ): void;
    onOpenFrameContextMenu?(frameId: string, position: { x: number; y: number }): void;
    onOpenEmptyCanvasContextMenu?(sheetId: string, position: { x: number; y: number }): void;
    onTransformPreview?(
      preview: PhotoTransformPreview | null,
    ): void;
    onTransformCommit(
      delta: PhotoTransformDelta,
    ): Promise<boolean>;
    onGraphicsUnavailable?(diagnostic: GraphicsDiagnostic): void;
  },
}));

interface ObservedViewport {
  callback: IntersectionObserverCallback;
  options: IntersectionObserverInit;
  targets: Set<Element>;
}

const observedViewports: ObservedViewport[] = [];

function emitPanelIntersections(
  rootMargin: string,
  intersections: Readonly<Record<string, boolean>>,
) {
  const viewport = observedViewports.find(
    ({ options }) => options.rootMargin === rootMargin,
  );
  if (!viewport) throw new Error(`Observer ${rootMargin} ausente.`);
  const entries = Object.entries(intersections).map(
    ([mediaId, isIntersecting]) => ({
      target: document.querySelector(`[data-media-id="${mediaId}"]`)!,
      isIntersecting,
    }),
  );
  viewport.callback(
    entries as IntersectionObserverEntry[],
    {} as IntersectionObserver,
  );
}

vi.mock("./AlbumCanvas", () => ({
  AlbumCanvas: (props: typeof canvasHarness.props) => {
    canvasHarness.props = props;
    return <div data-testid="album-canvas" />;
  },
}));

const projection = representativeProjection;
const twoSheetProjection = createTwoSheetProjection();
const decorativePreviewUrl =
  "asset://localhost/cache/decorative-overlay.png";
const decorativeProjection: EditorProjection = {
  ...projection,
  state: {
    ...projection.state,
    album: {
      ...projection.state.album,
      visualDefaults: {
        ...projection.state.album.visualDefaults,
        overlay: {
          scope: "bothSides",
          both: { kind: "media", mediaId: "decorative-overlay" },
        },
      },
      media: [
        ...projection.state.album.media,
        {
          id: "decorative-overlay",
          kind: "decorative",
          name: "Overlay translúcido.png",
          sourceWidthPx: 2_400,
          sourceHeightPx: 1_800,
          palette: ["#17344a", "#88b7c5", "#d4a15e"],
        },
      ],
    },
  },
  composition: {
    ...projection.composition,
    sheets: projection.composition.sheets.map((sheet) => ({
      ...sheet,
      overlays: [
        {
          mediaId: "decorative-overlay",
          name: "Overlay translúcido.png",
          drawRect: {
            x: 0,
            y: 0,
            width: sheet.widthUm,
            height: sheet.heightUm,
          },
        },
      ],
    })),
  },
  mediaUsage: [
    ...projection.mediaUsage,
    { mediaId: "decorative-overlay", count: 1 },
  ],
};

test.each([false, true])("applies a double-clicked Decorative to both sides of the implicit Sheet (Shift=%s)", async (shiftKey) => {
  const apply = vi.fn(async () => decorativeProjection);
  render(<ProjectWorkspace exportPipelinePort={exportPipelinePort} projection={decorativeProjection}
    projectCorePort={projectCorePortWithApply(apply)} onProjectionChange={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));
  fireEvent.doubleClick(within(screen.getByRole("group", { name: "Grade de Decorativos" })).getByRole("button", { name: /Overlay translúcido.png/ }), { shiftKey });
  await waitFor(() => expect(apply).toHaveBeenCalledExactlyOnceWith({
    kind: "applyDecorative", mediaId: "decorative-overlay", sheetId: decorativeProjection.state.album.sheets[0].id,
    role: shiftKey ? "overlay" : "background", scope: "bothSides",
  }, expect.any(Function)));
});

function deferredProjection() {
  let resolve!: (value: EditorProjection) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<EditorProjection>((resolver, rejecter) => {
    resolve = resolver;
    reject = rejecter;
  });
  return { promise, reject, resolve };
}

function deferredValue<Value>() {
  let resolve!: (value: Value) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<Value>((resolver, rejecter) => {
    resolve = resolver;
    reject = rejecter;
  });
  return { promise, reject, resolve };
}

const exportPipelinePort: ExportPipelinePort = {
  startSheet: () => ({
    completion: Promise.resolve({
      status: "completed",
      result: {
        widthPx: 600,
        heightPx: 300,
      },
    }),
    cancel: async () => "not_found",
  }),
};

interface LegacyExportPort {
  startSheet(
    sheetId: string,
    onEvent: (event: ExportProgressEvent) => void,
  ): ExportAttempt;
}

const exportPort: LegacyExportPort = {
  startSheet: (sheetId, onEvent) =>
    exportPipelinePort.startSheet(
      { projectName: "Projeto de teste", sheetId, sheetNumber: 1 },
      onEvent,
    ),
};

const inertProjectWindowPort: ProjectWindowPort = {
  onCloseRequested: async () => () => undefined,
  requestClose: async () => ({ kind: "closed" }),
  resolveClose: async () => ({ kind: "closed" }),
};

const inertProjectDialogPort: ProjectDialogPort = {
  acquire: () => ({
    dismiss: async () => undefined,
    present: async () => undefined,
  }),
};

function projectDialogHarness() {
  interface HarnessSession {
    active: boolean;
    closed: boolean;
    listener(action: ProjectDialogAction): void;
  }
  const sessions: HarnessSession[] = [];
  const dismiss = vi.fn(async () => undefined);
  const present = vi.fn<ProjectDialogSession["present"]>(
    async () => undefined,
  );
  const onAction = vi.fn(
    (_nextListener: (action: ProjectDialogAction) => void) => undefined,
  );
  const acquire: ProjectDialogPort["acquire"] = (nextListener) => {
    onAction(nextListener);
    const session: HarnessSession = {
      active: false,
      closed: false,
      listener: nextListener,
    };
    return {
      dismiss: async () => {
        await dismiss();
        if (session.closed) return;
        session.closed = true;
        const index = sessions.indexOf(session);
        if (index >= 0) sessions.splice(index, 1);
        if (session.active) {
          session.active = false;
          const next = sessions[0];
          if (next) next.active = true;
        }
      },
      present: async (state) => {
        await present(state);
        if (session.closed || sessions.includes(session)) return;
        sessions.push(session);
        if (sessions.length === 1) session.active = true;
      },
    };
  };
  return {
    dismiss,
    emit: (action: ProjectDialogAction) => {
      sessions.find((session) => session.active)?.listener(action);
    },
    onAction,
    port: { acquire } satisfies ProjectDialogPort,
    present,
  };
}

function projectWindowHarness() {
  let closeRequested: (() => void) | null = null;
  const port: ProjectWindowPort = {
    onCloseRequested: vi.fn(async (listener) => {
      closeRequested = listener;
      return () => {
        closeRequested = null;
      };
    }),
    requestClose: vi.fn(async () => ({
      kind: "confirmationRequired" as const,
    })),
    resolveClose: vi.fn(async () => ({ kind: "closed" as const })),
  };
  return {
    dialog: projectDialogHarness(),
    port,
    emitCloseRequested: () => closeRequested?.(),
  };
}

function projectCorePortWithApply(
  apply: ProjectCorePort["apply"],
): ProjectCorePort {
  return {
    load: async () => projection,
    validateAlbumInformation: async () => ({
      errors: [],
      impact: { sheetWidthPx: 7_087, pageWidthPx: 3_543, heightPx: 3_543 },
    }),
    apply,
    applyWithOutcome: async (intent, publish) => ({
      projection: await apply(intent, publish),
      affectedFrameId: "frame-001",
      affectedSheetId: null,
    }),
    importMedia: async () => ({ kind: "cancelled", projection }),
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    readSliderDoubleClickTime: async () => 500,
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Frame style preview is not configured in this fixture."); },
    previewDecorativeDrop: async () => { throw new Error("Decorative preview is not configured in this fixture."); },
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: async () => { throw new Error("Frame geometry preview is not configured in this fixture."); },
    resolvePhotoDropTarget: async () => ({ kind: "invalid" }),
    relink: async () => projection,
    undo: async () => projection,
    redo: async () => projection,
    save: async () => {
      throw new Error("Salvamento não configurado neste teste.");
    },
    saveAs: async () => {
      throw new Error("Salvar como não configurado neste teste.");
    },
  };
}

function projectSessionPortWithApply(
  apply: ProjectCorePort["apply"],
): ProjectCorePort {
  return projectCorePortWithApply(apply);
}

type TestProjectWorkspaceProps = Omit<
  ComponentProps<typeof ProjectWorkspaceView>,
  | "runProjectMutation"
  | "projectDialogPort"
  | "projectWindowPort"
  | "projectCorePort"
  | "exportPipelinePort"
  | "mediaPreviews"
  | "onGraphicsUnavailable"
  | "onMediaDemandChange"
  | "onRetryUnavailableMedia"
  | "onPreferencesReady"
  | "workspacePreferences"
> & {
  exportPipelinePort?: ExportPipelinePort;
  exportPort?: LegacyExportPort;
  mediaPreviews?: ComponentProps<typeof ProjectWorkspaceView>["mediaPreviews"];
  onGraphicsUnavailable?: ComponentProps<
    typeof ProjectWorkspaceView
  >["onGraphicsUnavailable"];
  onMediaDemandChange?: ComponentProps<
    typeof ProjectWorkspaceView
  >["onMediaDemandChange"];
  onPreferencesReady?: ComponentProps<
    typeof ProjectWorkspaceView
  >["onPreferencesReady"];
  projectCorePort?: ProjectCorePort;
  projectDialogPort?: ProjectDialogPort;
  projectSessionPort?: ProjectCorePort;
  projectWindowPort?: ProjectWindowPort;
  onRetryUnavailableMedia?: (mediaId: string) => Promise<void>;
  workspacePreferences?: ComponentProps<
    typeof ProjectWorkspaceView
  >["workspacePreferences"];
};

function ProjectWorkspace({
  exportPipelinePort: providedExportPipelinePort,
  exportPort: providedLegacyExportPort,
  mediaPreviews = {},
  onGraphicsUnavailable = () => undefined,
  onMediaDemandChange = () => undefined,
  onPreferencesReady = () => undefined,
  projectDialogPort = inertProjectDialogPort,
  projectCorePort: providedProjectCorePort,
  projectSessionPort,
  projectWindowPort = inertProjectWindowPort,
  onRetryUnavailableMedia = async () => undefined,
  projection,
  workspacePreferences = { kind: "memory" },
  ...props
}: TestProjectWorkspaceProps) {
  const projectCorePort =
    providedProjectCorePort ??
    projectSessionPort ??
    projectCorePortWithApply(async () => projection);
  const effectiveExportPipelinePort =
    providedExportPipelinePort ??
    (providedLegacyExportPort
      ? {
          startSheet: (selection, onEvent) =>
            providedLegacyExportPort.startSheet(selection.sheetId, onEvent),
        }
      : exportPipelinePort);
  const runProjectMutation = useProjectMutationRunner(
    projection.state.projectId,
    projectCorePort,
  );
  return (
    <ProjectWorkspaceView
      {...props}
      exportPipelinePort={effectiveExportPipelinePort}
      mediaPreviews={mediaPreviews}
      onGraphicsUnavailable={onGraphicsUnavailable}
      onMediaDemandChange={onMediaDemandChange}
      onPreferencesReady={onPreferencesReady}
      projectDialogPort={projectDialogPort}
      projection={projection}
      projectCorePort={projectCorePort}
      projectWindowPort={projectWindowPort}
      runProjectMutation={runProjectMutation}
      workspacePreferences={workspacePreferences}
      onRetryUnavailableMedia={onRetryUnavailableMedia}
    />
  );
}

function getApplicationCommand(
  menuName:
    | "Arquivo"
    | "Editar"
    | "Lâmina"
    | "Exibir"
    | "Ferramentas"
    | "Ajuda",
  commandName: string,
) {
  const menu = screen.getByRole("menuitem", { name: menuName });
  if (menu.getAttribute("aria-expanded") !== "true") {
    fireEvent.click(menu);
  }
  return (
    screen.queryByRole("menuitem", { name: commandName }) ??
    screen.getByRole("menuitemcheckbox", { name: commandName })
  );
}

function performGridPointerReorder(
  view: ReturnType<typeof render>,
  sourceIndex: number,
  targetIndex: number,
) {
  const slots = Array.from(
    view.container.querySelectorAll<HTMLElement>(".sheet-grid-slot"),
  );
  const source = slots[sourceIndex]!;
  const pressTarget = source.querySelector<HTMLElement>(".sheet-tile")!;
  const surface = screen.getByTestId("sheet-reorder-grid");
  const reorderEnabled = source.hasAttribute("data-reorder-enabled");
  const pointerId = gridPointerId(sourceIndex, targetIndex);
  arrangeWorkspaceGrid(view, slots);
  Object.defineProperties(surface, {
    releasePointerCapture: { configurable: true, value: vi.fn() },
    setPointerCapture: { configurable: true, value: vi.fn() },
  });
  pressTarget.addEventListener(
    "pointerdown",
    (event) => event.stopPropagation(),
    { once: true },
  );
  fireEvent.pointerDown(pressTarget, {
    button: 0,
    buttons: 1,
    clientX: 50,
    clientY: sourceIndex * 110 + 50,
    pointerId,
    pointerType: "mouse",
  });
  if (reorderEnabled) {
    expect(surface.setPointerCapture).toHaveBeenCalledWith(pointerId);
  } else {
    expect(surface.setPointerCapture).not.toHaveBeenCalled();
  }
  fireEvent.pointerMove(surface, {
    buttons: 1,
    clientX: 50,
    clientY: targetIndex * 110 + 50,
    pointerId,
    pointerType: "mouse",
  });
}

function finishGridPointerReorder(sourceIndex: number, targetIndex: number) {
  const surface = screen.getByTestId("sheet-reorder-grid");
  fireEvent.pointerUp(surface, {
    button: 0,
    buttons: 0,
    clientX: 50,
    clientY: targetIndex * 110 + 50,
    pointerId: gridPointerId(sourceIndex, targetIndex),
    pointerType: "mouse",
  });
}

function arrangeWorkspaceGrid(
  view: ReturnType<typeof render>,
  slots: readonly HTMLElement[],
) {
  slots.forEach((slot, index) => {
    Object.defineProperty(slot, "getBoundingClientRect", {
      configurable: true,
      value: () => workspaceRect(0, index * 110, 100, index * 110 + 100),
    });
  });
  const grid = screen.getByTestId("sheet-reorder-grid");
  Object.defineProperty(grid, "getBoundingClientRect", {
    configurable: true,
    value: () => workspaceRect(0, 0, 220, slots.length * 110),
  });
  const viewport = view.container.querySelector<HTMLElement>(
    ".inspector-scroll",
  )!;
  Object.defineProperty(viewport, "getBoundingClientRect", {
    configurable: true,
    value: () => workspaceRect(0, 0, 220, slots.length * 110),
  });
}

function gridPointerId(sourceIndex: number, targetIndex: number): number {
  return 500 + sourceIndex * 10 + targetIndex;
}

function workspaceRect(
  left: number,
  top: number,
  right: number,
  bottom: number,
): DOMRect {
  return {
    bottom,
    height: bottom - top,
    left,
    right,
    top,
    width: right - left,
    x: left,
    y: top,
    toJSON: () => ({}),
  };
}

beforeEach(() => {
  canvasHarness.props = null;
  localStorage.clear();
  observedViewports.length = 0;
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      readonly viewport: ObservedViewport;

      constructor(
        callback: IntersectionObserverCallback,
        options: IntersectionObserverInit = {},
      ) {
        this.viewport = { callback, options, targets: new Set() };
        observedViewports.push(this.viewport);
      }

      observe(target: Element) {
        this.viewport.targets.add(target);
      }

      unobserve(target: Element) {
        this.viewport.targets.delete(target);
      }

      disconnect() {
        this.viewport.targets.clear();
      }

      takeRecords() {
        return [];
      }
    },
  );
  useEditorView.setState({
    projectId: projection.state.projectId,
    selectedFrameIds: [],
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    editingSheetId: null,
    viewport: { offsetX: 42 },
  });
});

test("creates one placeholder from Edit and selects only the new Frame", async () => {
  const added = structuredClone(projection);
  added.state.album.sheets[0].frames.push({
    ...added.state.album.sheets[0].frames[0], id: "manual-frame", zIndex: 1, photo: null,
  });
  added.composition.sheets[0].frames.push({
    ...added.composition.sheets[0].frames[0], frameId: "manual-frame", zIndex: 1, photo: null,
  });
  useEditorView.setState({ editingSheetId: "sheet-001", selectedFrameIds: ["frame-001"] });
  const port = projectCorePortWithApply(async () => added);
  port.applyWithOutcome = vi.fn(async () => ({ projection: added,
    affectedFrameId: "manual-frame", affectedSheetId: null }));
  render(<ProjectWorkspace projection={projection} projectCorePort={port} onProjectionChange={vi.fn()} />);
  fireEvent.click(getApplicationCommand("Editar", "Adicionar Frame"));
  await waitFor(() => expect(port.applyWithOutcome).toHaveBeenCalledWith(
    { kind: "addFrame", sheetId: "sheet-001" }, expect.any(Function)));
  expect(useEditorView.getState().selectedFrameIds).toEqual(["manual-frame"]);
});

test("empty Canvas context creates a Frame without requiring a previous selection", async () => {
  useEditorView.setState({ editingSheetId: "sheet-001", selectedFrameIds: [] });
  const port = projectCorePortWithApply(async () => projection);
  port.applyWithOutcome = vi.fn(async () => ({ projection, affectedFrameId: "frame-001", affectedSheetId: null }));
  render(<ProjectWorkspace projection={projection} projectCorePort={port} onProjectionChange={vi.fn()} />);
  act(() => canvasHarness.props?.onOpenEmptyCanvasContextMenu?.("sheet-001", { x: 120, y: 180 }));
  const menu = screen.getByRole("menu", { name: "Área vazia do Canvas" });
  expect(within(menu).getAllByRole("menuitem")).toHaveLength(1);
  expect(useEditorView.getState().selectedFrameIds).toEqual([]);
  fireEvent.click(within(menu).getByRole("menuitem", { name: "Adicionar Frame" }));
  await waitFor(() => expect(port.applyWithOutcome).toHaveBeenCalledWith(
    { kind: "addFrame", sheetId: "sheet-001" }, expect.any(Function)));
  expect(screen.queryByRole("menu", { name: "Área vazia do Canvas" })).not.toBeInTheDocument();
  expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-001"]);
});

test("manual Frame creation is unavailable outside sheet editing", () => {
  const port = projectCorePortWithApply(async () => projection);
  port.applyWithOutcome = vi.fn();
  render(<ProjectWorkspace projection={projection} projectCorePort={port} onProjectionChange={vi.fn()} />);
  expect(getApplicationCommand("Editar", "Adicionar Frame")).toBeDisabled();
  fireEvent.keyDown(screen.getByRole("menu", { name: "Editar" }), { key: "Escape" });
  act(() => canvasHarness.props?.onOpenEmptyCanvasContextMenu?.("sheet-001", { x: 120, y: 180 }));
  expect(screen.queryByRole("menu", { name: "Área vazia do Canvas" })).not.toBeInTheDocument();
  expect(port.applyWithOutcome).not.toHaveBeenCalled();
});

test.each(["keyboard", "context"])("deletes the entire Frame selection via %s without confirmation", async (entry) => {
  const grouped = structuredClone(projection);
  grouped.state.album.sheets[0].frames.push({
    ...grouped.state.album.sheets[0].frames[0], id: "frame-002", zIndex: 1, photo: null,
  });
  grouped.composition.sheets[0].frames.push({
    ...grouped.composition.sheets[0].frames[0], frameId: "frame-002", zIndex: 1, photo: null,
  });
  const deleted = structuredClone(grouped);
  deleted.state.album.sheets[0].frames = [];
  deleted.composition.sheets[0].frames = [];
  useEditorView.setState({ editingSheetId: "sheet-001", selectedFrameIds: ["frame-001", "frame-002"] });
  const apply = vi.fn(async () => deleted);
  render(<ProjectWorkspace projection={grouped} projectCorePort={projectCorePortWithApply(apply)} onProjectionChange={vi.fn()} />);
  if (entry === "keyboard") fireEvent.keyDown(window, { key: "Delete" });
  else {
    act(() => canvasHarness.props?.onOpenFrameContextMenu?.("frame-002", { x: 320, y: 200 }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Excluir" }));
  }
    await waitFor(() => expect(apply).toHaveBeenCalledWith({ kind: "deleteFrames", frameIds: ["frame-001", "frame-002"], mode: "edit" }, expect.any(Function)));
  expect(apply).toHaveBeenCalledOnce();
  expect(useEditorView.getState().selectedFrameIds).toEqual([]);
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

test.each([
  ["photos", "edit"], ["photos", "context"], ["placeholder", "edit"], ["placeholder", "context"],
])("swaps %s through %s and preserves the selected pair", async (name, entry) => {
  const swap = frameContentSwapCorpus.cases.find((item) => item.name === name)!;
  const initial = structuredClone(frameContentSwapCorpus.before);
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: "sheet-001", selectedFrameIds: swap.selectedFrameIds });
  const apply = vi.fn(async () => structuredClone(swap.after));
  render(<ProjectWorkspace projection={initial} projectCorePort={projectCorePortWithApply(apply)} onProjectionChange={vi.fn()} />);
  if (entry === "edit") {
    const command = getApplicationCommand("Editar", "Trocar conteúdo dos Frames");
    expect(command).toBeEnabled();
    expect(command).not.toHaveAttribute("data-placeholder-feature");
    fireEvent.click(command);
  } else {
    act(() => canvasHarness.props?.onOpenFrameContextMenu?.(swap.selectedFrameIds[1], { x: 320, y: 200 }));
    const command = screen.getByRole("menuitem", { name: "Trocar conteúdo dos Frames" });
    expect(command).toBeEnabled();
    fireEvent.click(command);
  }
  await waitFor(() => expect(apply).toHaveBeenCalledWith({ kind: "swapFrameContents", frameIds: swap.selectedFrameIds }, expect.any(Function)));
  expect(apply).toHaveBeenCalledOnce();
  expect(useEditorView.getState().selectedFrameIds).toEqual(swap.selectedFrameIds);
  expect(screen.queryByRole("menu", { name: "Organizar Frames" })).not.toBeInTheDocument();
});

test.each([
  ["swap-frame-0"], ["swap-frame-0", "swap-frame-1", "swap-frame-2"], ["swap-frame-2", "swap-frame-3"],
])("keeps swap disabled in both menus for incompatible selection %j", (...selectedFrameIds) => {
  const initial = structuredClone(frameContentSwapCorpus.before);
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: "sheet-001", selectedFrameIds });
  const apply = vi.fn(async () => initial);
  render(<ProjectWorkspace projection={initial} projectCorePort={projectCorePortWithApply(apply)} onProjectionChange={vi.fn()} />);
  expect(getApplicationCommand("Editar", "Trocar conteúdo dos Frames")).toBeDisabled();
  fireEvent.keyDown(screen.getByRole("menu", { name: "Editar" }), { key: "Escape" });
  act(() => canvasHarness.props?.onOpenFrameContextMenu?.(selectedFrameIds[0], { x: 320, y: 200 }));
  const command = screen.getByRole("menuitem", { name: "Trocar conteúdo dos Frames" });
  expect(command).toBeDisabled();
  fireEvent.click(command);
  expect(apply).not.toHaveBeenCalled();
});

test("arranges the entire Frame selection from Edit without changing the selection", async () => {
  const grouped = structuredClone(projection);
  grouped.state.album.sheets[0].frames.push({
    ...grouped.state.album.sheets[0].frames[0], id: "frame-002", zIndex: 1, photo: null,
  });
  grouped.composition.sheets[0].frames.push({
    ...grouped.composition.sheets[0].frames[0], frameId: "frame-002", zIndex: 1, photo: null,
  });
  useEditorView.setState({ editingSheetId: "sheet-001", selectedFrameIds: ["frame-002", "frame-001"] });
  const apply = vi.fn(async (_intent: ProjectIntent) => grouped);
  render(<ProjectWorkspace projection={grouped} projectCorePort={projectCorePortWithApply(apply)}
    onProjectionChange={vi.fn()} />);
  for (const [label, action] of [
    ["Trazer para frente", "bringToFront"], ["Avançar uma posição", "advance"],
    ["Recuar uma posição", "recede"], ["Enviar para trás", "sendToBack"],
  ]) {
    fireEvent.click(getApplicationCommand("Editar", "Organizar"));
    fireEvent.click(screen.getByRole("menuitem", { name: label }));
    await waitFor(() => expect(apply).toHaveBeenLastCalledWith({
      kind: "arrangeFrames", frameIds: ["frame-002", "frame-001"], action,
    }, expect.any(Function)));
    expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-002", "frame-001"]);
  }
});

test("opens Frame context actions for the clicked selection and preserves it after arranging", async () => {
  const grouped = structuredClone(projection);
  grouped.state.album.sheets[0].frames.push({
    ...grouped.state.album.sheets[0].frames[0], id: "frame-002", zIndex: 1, photo: null,
  });
  grouped.composition.sheets[0].frames.push({
    ...grouped.composition.sheets[0].frames[0], frameId: "frame-002", zIndex: 1, photo: null,
  });
  useEditorView.setState({ editingSheetId: "sheet-001", selectedFrameIds: ["frame-001", "frame-002"] });
  const apply = vi.fn(async (_intent: ProjectIntent) => grouped);
  render(<ProjectWorkspace projection={grouped} projectCorePort={projectCorePortWithApply(apply)}
    onProjectionChange={vi.fn()} />);
  act(() => canvasHarness.props?.onOpenFrameContextMenu?.("frame-002", { x: 120, y: 180 }));
  const menu = screen.getByRole("menu", { name: "Organizar Frames" });
  expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-001", "frame-002"]);
  fireEvent.click(within(menu).getByRole("menuitem", { name: "Enviar para trás" }));
  await waitFor(() => expect(apply).toHaveBeenLastCalledWith({
    kind: "arrangeFrames", frameIds: ["frame-001", "frame-002"], action: "sendToBack",
  }, expect.any(Function)));
  expect(screen.queryByRole("menu", { name: "Organizar Frames" })).not.toBeInTheDocument();
  act(() => useEditorView.getState().selectFrame("frame-001"));
  act(() => canvasHarness.props?.onOpenFrameContextMenu?.("frame-002", { x: 120, y: 180 }));
  expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-002"]);
  fireEvent.keyDown(screen.getByRole("menu", { name: "Organizar Frames" }), { key: "Escape" });
  expect(screen.queryByRole("menu", { name: "Organizar Frames" })).not.toBeInTheDocument();
});

test("presents the canonical desktop menus and marks unfinished commands", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  for (const menuName of [
    "Arquivo",
    "Editar",
    "Lâmina",
    "Exibir",
    "Ferramentas",
    "Ajuda",
  ]) {
    expect(screen.getByRole("menuitem", { name: menuName })).toBeEnabled();
  }
  expect(screen.queryByRole("button", { name: "Inserir" })).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Visualizar" }),
  ).not.toBeInTheDocument();

  expect(getApplicationCommand("Arquivo", "Salvar")).toBeEnabled();
  const newProject = getApplicationCommand("Arquivo", "Novo Projeto…");
  expect(newProject).toBeDisabled();
  expect(newProject).toHaveAttribute(
    "data-placeholder-feature",
    "new-project-from-project-window",
  );

  expect(getApplicationCommand("Editar", "Adicionar Frame")).toBeDisabled();
  expect(getApplicationCommand("Editar", "Copiar")).toBeDisabled();
  expect(
    screen.queryByRole("menuitem", { name: "Copiar Frames" }),
  ).not.toBeInTheDocument();
  expect(
    getApplicationCommand("Editar", "Trocar conteúdo dos Frames"),
  ).toBeDisabled();
  expect(
    getApplicationCommand("Editar", "Salvar disposição como Layout"),
  ).toBeDisabled();
  const arrange = getApplicationCommand("Editar", "Organizar");
  expect(arrange).toHaveAttribute("aria-haspopup", "menu");
  fireEvent.click(arrange);
  expect(
    screen.getByRole("menuitem", { name: "Trazer para frente" }),
  ).toBeDisabled();
  const addSheetAfter = getApplicationCommand("Lâmina", "Adicionar depois");
  expect(addSheetAfter).toBeEnabled();
  expect(addSheetAfter).not.toHaveAttribute("data-placeholder-feature");
  expect(getApplicationCommand("Exibir", "Painel de imagens")).toBeEnabled();
  expect(getApplicationCommand("Exibir", "Painel de imagens")).toHaveAttribute(
    "aria-checked",
    "true",
  );
  expect(getApplicationCommand("Ferramentas", "Configurações…")).toBeDisabled();
  expect(getApplicationCommand("Ajuda", "Manual do MyAlbuns")).toBeDisabled();
});

test("routes implicit menu and explicit context actions to their intended Sheets", async () => {
  const physicalProjection = createThreeSheetProjection();
  const port = projectCorePortWithApply(async () => physicalProjection);
  const applyWithOutcome = vi.fn(async () => ({
    projection: physicalProjection,
    affectedFrameId: null,
    affectedSheetId: null,
  }));
  port.applyWithOutcome = applyWithOutcome;

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={physicalProjection}
      projectCorePort={port}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => canvasHarness.props?.onCenteredSheetChange?.("sheet-002"));
  fireEvent.click(getApplicationCommand("Lâmina", "Adicionar depois"));
  await waitFor(() =>
    expect(applyWithOutcome).toHaveBeenCalledWith({
      kind: "addSheet",
      anchorSheetId: "sheet-002",
      position: "after",
    }, expect.any(Function)),
  );

  act(() =>
    canvasHarness.props?.onOpenSheetContextMenu?.("sheet-003", {
      x: 240,
      y: 180,
    }),
  );
  const contextMenu = screen.getByRole("menu", {
    name: "Ações da Lâmina 03",
  });
  expect(contextMenu).toHaveStyle({ left: "240px", top: "180px" });
  fireEvent.click(
    within(contextMenu).getByRole("menuitem", { name: "Excluir" }),
  );

  await waitFor(() =>
    expect(applyWithOutcome).toHaveBeenLastCalledWith({
      kind: "deleteSheet",
      sheetId: "sheet-003",
    }, expect.any(Function)),
  );
  expect(
    screen.queryByRole("menu", { name: "Ações da Lâmina 03" }),
  ).not.toBeInTheDocument();
});

test("opens and dismisses an explicit Sheet context menu without navigating the Canvas", () => {
  const physicalProjection = createThreeSheetProjection();
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={physicalProjection}
      projectCorePort={projectCorePortWithApply(async () => physicalProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCanvasMetricsChange?.({
      width: 1_000,
      height: 500,
      scale: 0.5,
    });
  });
  fireEvent.click(
    screen.getByRole("button", { name: /Ir para Lâmina 02/u }),
  );
  const before = {
    centeredSheetId: useEditorView.getState().centeredSheetId,
    focusedSheetId: useEditorView.getState().focusedSheetId,
    selectedFrameIds: useEditorView.getState().selectedFrameIds,
    viewport: { ...useEditorView.getState().viewport },
  };

  act(() =>
    canvasHarness.props?.onOpenSheetContextMenu?.("sheet-003", {
      x: 240,
      y: 180,
    }),
  );
  expect(
    screen.getByRole("menu", { name: "Ações da Lâmina 03" }),
  ).toBeInTheDocument();
  expect(useEditorView.getState()).toMatchObject(before);

  fireEvent.keyDown(document, { key: "Escape" });
  expect(
    screen.queryByRole("menu", { name: "Ações da Lâmina 03" }),
  ).not.toBeInTheDocument();
  expect(useEditorView.getState()).toMatchObject(before);

  act(() =>
    canvasHarness.props?.onOpenSheetContextMenu?.("sheet-003", {
      x: 240,
      y: 180,
    }),
  );
  fireEvent.pointerDown(document.body);
  expect(
    screen.queryByRole("menu", { name: "Ações da Lâmina 03" }),
  ).not.toBeInTheDocument();
  expect(useEditorView.getState()).toMatchObject(before);

  act(() =>
    canvasHarness.props?.onOpenSheetContextMenu?.("sheet-003", {
      x: 240,
      y: 180,
    }),
  );
  const dismissLayer = document.querySelector(
    ".ui-context-menu__dismiss-layer",
  );
  expect(dismissLayer).toBeInstanceOf(HTMLElement);
  fireEvent.pointerDown(dismissLayer!, { button: 0, pointerId: 31 });
  fireEvent.pointerUp(dismissLayer!, { button: 0, pointerId: 31 });
  expect(
    screen.getByRole("menu", { name: "Ações da Lâmina 03" }),
  ).toBeInTheDocument();
  expect(useEditorView.getState()).toMatchObject(before);
  fireEvent.click(dismissLayer!);
  expect(
    screen.queryByRole("menu", { name: "Ações da Lâmina 03" }),
  ).not.toBeInTheDocument();
  expect(useEditorView.getState()).toMatchObject(before);
});

test("selects a Sheet through the Bar seam without navigating the Canvas", () => {
  const physicalProjection = createThreeSheetProjection();
  physicalProjection.state.album.sheets[0]!.activeSides = "right";
  physicalProjection.state.album.sheets[0]!.pageNumbers = [1];
  physicalProjection.composition.sheets[0]!.activeSides = "right";
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={physicalProjection}
      projectCorePort={projectCorePortWithApply(async () => physicalProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCanvasMetricsChange?.({
      width: 1_000,
      height: 500,
      scale: 0.5,
    });
  });
  fireEvent.click(
    screen.getByRole("button", { name: /Ir para Lâmina 02/u }),
  );
  const before = {
    centeredSheetId: canvasHarness.props?.centeredSheetId,
    offsetX: canvasHarness.props?.viewport.offsetX,
  };
  expect(before).toMatchObject({ centeredSheetId: "sheet-002" });
  expect(before.offsetX).not.toBe(0);

  for (const sheetId of ["sheet-001", "sheet-003", "sheet-003"]) {
    act(() => canvasHarness.props?.sheetReorder?.onSelect(sheetId));

    expect(canvasHarness.props?.focusedSheetId).toBe(sheetId);
    expect(canvasHarness.props?.centeredSheetId).toBe(before.centeredSheetId);
    expect(canvasHarness.props?.viewport.offsetX).toBe(before.offsetX);
  }
});

test("routes Delete to the centered Sheet and guards text entry, Edit Mode, and the minimum", async () => {
  const physicalProjection = createThreeSheetProjection();
  const port = projectCorePortWithApply(async () => physicalProjection);
  const applyWithOutcome = vi.fn(async () => ({
    projection: physicalProjection,
    affectedFrameId: null,
    affectedSheetId: null,
  }));
  port.applyWithOutcome = applyWithOutcome;
  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={physicalProjection}
      projectCorePort={port}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => canvasHarness.props?.onCenteredSheetChange?.("sheet-002"));
  fireEvent.keyDown(window, { key: "Delete" });
  await waitFor(() =>
    expect(applyWithOutcome).toHaveBeenCalledWith({
      kind: "deleteSheet",
      sheetId: "sheet-002",
    }, expect.any(Function)),
  );

  const media = screen.getByRole("button", { name: /Campo\.jpg/ });
  media.focus();
  fireEvent.keyDown(media, { key: "Delete" });
  expect(applyWithOutcome).toHaveBeenCalledOnce();

  act(() => useEditorView.getState().selectFrame("frame-001"));
  fireEvent.keyDown(window, { key: "Delete" });
  expect(applyWithOutcome).toHaveBeenCalledOnce();
  act(() => useEditorView.getState().selectFrame(null));

  const input = document.createElement("input");
  document.body.append(input);
  try {
    fireEvent.keyDown(input, { key: "Delete" });
    expect(applyWithOutcome).toHaveBeenCalledOnce();
  } finally {
    input.remove();
  }

  act(() => canvasHarness.props?.onEditSheet?.("sheet-002"));
  fireEvent.keyDown(window, { key: "Delete" });
  expect(applyWithOutcome).toHaveBeenCalledOnce();
  view.unmount();

  const minimumProjection = createTwoSheetProjection();
  const minimumPort = projectCorePortWithApply(async () => minimumProjection);
  const minimumApply = vi.fn(async () => ({
    projection: minimumProjection,
    affectedFrameId: null,
    affectedSheetId: null,
  }));
  minimumPort.applyWithOutcome = minimumApply;
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={minimumProjection}
      projectCorePort={minimumPort}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.keyDown(window, { key: "Delete" });
  expect(minimumApply).not.toHaveBeenCalled();
});

test.each(["completed", "failed"] as const)(
  "blocks adjacent structural commands while a predecessor is pending and then %s",
  async (predecessorOutcome) => {
    const physicalProjection = createThreeSheetProjection();
    const pendingDelete = deferredValue<
      Awaited<ReturnType<ProjectCorePort["applyWithOutcome"]>>
    >();
    const port = projectCorePortWithApply(async () => physicalProjection);
    port.applyWithOutcome = vi.fn(() => pendingDelete.promise);
    render(
      <ProjectWorkspace
        exportPipelinePort={exportPipelinePort}
        projection={physicalProjection}
        projectCorePort={port}
        onProjectionChange={() => undefined}
      />,
    );

    fireEvent.click(getApplicationCommand("Lâmina", "Excluir"));
    await waitFor(() => expect(port.applyWithOutcome).toHaveBeenCalledOnce());

    const adjacentCommand = getApplicationCommand(
      "Lâmina",
      "Adicionar depois",
    );
    expect(adjacentCommand).toBeDisabled();
    fireEvent.click(adjacentCommand);

    await act(async () => {
      if (predecessorOutcome === "completed") {
        pendingDelete.resolve({
          projection: physicalProjection,
          affectedFrameId: null,
          affectedSheetId: null,
        });
      } else {
        pendingDelete.reject(new Error("Falha estrutural controlada."));
      }
      await pendingDelete.promise.catch(() => undefined);
    });

    await waitFor(() => expect(port.applyWithOutcome).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(
        getApplicationCommand("Lâmina", "Adicionar depois"),
      ).toBeEnabled(),
    );
  },
);

type QueuedStructuralIntent = Exclude<
  SheetStructureIntent,
  { kind: "reorderSheet" }
>;

const queuedHistoryStructuralCases = (["undo", "redo"] as const).flatMap(
  (historyOperation) =>
    (["completed", "failed"] as const).flatMap((historyOutcome) =>
      [
        {
          commandName: "Adicionar depois",
          intent: {
            kind: "addSheet",
            anchorSheetId: "sheet-003",
            position: "after",
          } satisfies QueuedStructuralIntent,
          operationName: "add",
        },
        {
          commandName: "Excluir",
          intent: {
            kind: "deleteSheet",
            sheetId: "sheet-003",
          } satisfies QueuedStructuralIntent,
          operationName: "delete",
        },
        {
          commandName: "Converter extremidade",
          intent: {
            kind: "convertEdgeSheet",
            sheetId: "sheet-003",
          } satisfies QueuedStructuralIntent,
          operationName: "convert",
        },
      ].map((structural) => ({
        historyOperation,
        historyOutcome,
        ...structural,
      })),
    ),
);

test.each(queuedHistoryStructuralCases)(
  "keeps the latest projection authoritative for delayed $historyOperation $historyOutcome followed by $operationName",
  async ({ commandName, historyOperation, historyOutcome, intent }) => {
    const beforeHistory = createThreeSheetProjection();
    beforeHistory.state.canUndo = historyOperation === "undo";
    beforeHistory.state.canRedo = historyOperation === "redo";
    const afterHistory = createTwoSheetProjection();
    afterHistory.state.revision = beforeHistory.state.revision + 1;
    afterHistory.state.canUndo = historyOperation === "redo";
    afterHistory.state.canRedo = historyOperation === "undo";
    const pendingHistory = deferredProjection();
    let authoritativeProjection = beforeHistory;
    const port = projectCorePortWithApply(async () => authoritativeProjection);
    const history = vi.fn(() => pendingHistory.promise);
    port[historyOperation] = history;
    const applyWithOutcome = vi.fn<ProjectCorePort["applyWithOutcome"]>(
      async (queuedIntent) => {
        const sheetId =
          queuedIntent.kind === "addSheet"
            ? queuedIntent.anchorSheetId
            : queuedIntent.kind === "deleteSheet" ||
                queuedIntent.kind === "convertEdgeSheet"
              ? queuedIntent.sheetId
              : null;
        if (
          sheetId &&
          !authoritativeProjection.state.album.sheets.some(
            (sheet) => sheet.id === sheetId,
          )
        ) {
          throw new Error(`Lâmina não encontrada: ${sheetId}`);
        }
        return {
          projection: authoritativeProjection,
          affectedFrameId: null,
          affectedSheetId: sheetId,
        };
      },
    );
    port.applyWithOutcome = applyWithOutcome;
    const dialog = projectDialogHarness();
    const onProjectionChange = vi.fn();

    render(
      <ProjectWorkspace
        exportPipelinePort={exportPipelinePort}
        projection={beforeHistory}
        projectCorePort={port}
        projectDialogPort={dialog.port}
        onProjectionChange={onProjectionChange}
      />,
    );

    act(() => canvasHarness.props?.onCenteredSheetChange?.("sheet-003"));
    fireEvent.keyDown(window, {
      ctrlKey: true,
      key: historyOperation === "undo" ? "z" : "y",
    });
    await waitFor(() => expect(history).toHaveBeenCalledOnce());
    fireEvent.click(getApplicationCommand("Lâmina", commandName));
    expect(applyWithOutcome).not.toHaveBeenCalled();

    await act(async () => {
      if (historyOutcome === "completed") {
        authoritativeProjection = afterHistory;
        pendingHistory.resolve(afterHistory);
      } else {
        pendingHistory.reject(
          new Error(`${historyOperation} indisponível para este teste.`),
        );
      }
      await pendingHistory.promise.catch(() => undefined);
    });

    await waitFor(() =>
      expect(onProjectionChange).toHaveBeenCalledTimes(
        historyOutcome === "completed" ? 2 : 1,
      ),
    );
    if (historyOutcome === "completed") {
      expect(applyWithOutcome).not.toHaveBeenCalled();
    } else {
      expect(applyWithOutcome).toHaveBeenCalledOnce();
      expect(applyWithOutcome).toHaveBeenCalledWith(intent, expect.any(Function));
    }
    expect(dialog.present).not.toHaveBeenCalledWith(
      expect.objectContaining({ kind: "projectOperationFailure" }),
    );
  },
);

test("routes implicit and explicit empty-edge conversions to their intended Sheet", async () => {
  const physicalProjection = createThreeSheetProjection();
  const port = projectCorePortWithApply(async () => physicalProjection);
  const applyWithOutcome = vi.fn(async () => ({
    projection: physicalProjection,
    affectedFrameId: null,
    affectedSheetId: "sheet-003",
  }));
  port.applyWithOutcome = applyWithOutcome;

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={physicalProjection}
      projectCorePort={port}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => canvasHarness.props?.onCenteredSheetChange?.("sheet-003"));
  const implicitCommand = getApplicationCommand(
    "Lâmina",
    "Converter extremidade",
  );
  expect(implicitCommand).toBeEnabled();
  fireEvent.click(implicitCommand);
  await waitFor(() =>
    expect(applyWithOutcome).toHaveBeenCalledWith({
      kind: "convertEdgeSheet",
      sheetId: "sheet-003",
    }, expect.any(Function)),
  );

  act(() =>
    canvasHarness.props?.onOpenSheetContextMenu?.("sheet-003", {
      x: 240,
      y: 180,
    }),
  );
  const contextMenu = screen.getByRole("menu", {
    name: "Ações da Lâmina 03",
  });
  const explicitCommand = within(contextMenu).getByRole("menuitem", {
    name: "Converter extremidade",
  });
  expect(explicitCommand).toBeEnabled();
  fireEvent.click(explicitCommand);
  await waitFor(() => expect(applyWithOutcome).toHaveBeenCalledTimes(2));
  expect(applyWithOutcome).toHaveBeenLastCalledWith({
    kind: "convertEdgeSheet",
    sheetId: "sheet-003",
  }, expect.any(Function));
});

test("commits one valid Grade reorder and keeps structural controls inert in Sheet Edit Mode", async () => {
  const physicalProjection = createThreeSheetProjection();
  const port = projectCorePortWithApply(async () => physicalProjection);
  const applyWithOutcome = vi.fn(async () => ({
    projection: physicalProjection,
    affectedFrameId: null,
    affectedSheetId: "sheet-001",
  }));
  port.applyWithOutcome = applyWithOutcome;
  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={physicalProjection}
      projectCorePort={port}
      onProjectionChange={() => undefined}
    />,
  );

  performGridPointerReorder(view, 0, 1);
  expect(screen.getByTestId("sheet-reorder-grid")).toHaveAttribute(
    "data-reorder-state",
    "preview",
  );
  finishGridPointerReorder(0, 1);

  await waitFor(() =>
    expect(applyWithOutcome).toHaveBeenCalledWith({
      kind: "reorderSheet",
      sheetId: "sheet-001",
      targetIndex: 1,
    }, expect.any(Function)),
  );
  expect(applyWithOutcome).toHaveBeenCalledOnce();

  act(() => canvasHarness.props?.onEditSheet?.("sheet-001"));
  expect(getApplicationCommand("Lâmina", "Adicionar depois")).toBeDisabled();
  expect(getApplicationCommand("Lâmina", "Excluir")).toBeDisabled();
  expect(
    getApplicationCommand("Lâmina", "Converter extremidade"),
  ).toBeDisabled();
  for (const slot of view.container.querySelectorAll(".sheet-grid-slot")) {
    expect(slot).not.toHaveAttribute("draggable");
    expect(slot).not.toHaveAttribute("data-reorder-enabled");
  }
  act(() =>
    canvasHarness.props?.onOpenSheetContextMenu?.("sheet-002", {
      x: 40,
      y: 50,
    }),
  );
  expect(
    screen.queryByRole("menu", { name: "Ações da Lâmina 02" }),
  ).not.toBeInTheDocument();
});

test("previews a Bar reorder locally while the Grade stays confirmed, then commits once", async () => {
  const physicalProjection = createThreeSheetProjection();
  const port = projectCorePortWithApply(async () => physicalProjection);
  const applyWithOutcome = vi.fn(async () => ({
    projection: physicalProjection,
    affectedFrameId: null,
    affectedSheetId: "sheet-001",
  }));
  port.applyWithOutcome = applyWithOutcome;
  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={physicalProjection}
      projectCorePort={port}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => canvasHarness.props?.sheetReorder?.onPreview("sheet-001", 1));

  expect(canvasHarness.props?.sheetReorder).toMatchObject({
    status: "preview",
    representation: {
      order: ["sheet-002", "sheet-001", "sheet-003"],
      placeholderIndex: 1,
      ghost: { sheetId: "sheet-001" },
    },
  });
  expect(
    Array.from(
      view.container.querySelectorAll<HTMLElement>(".sheet-grid-slot"),
      (slot) => slot.dataset.sheetId,
    ),
  ).toEqual(["sheet-001", "sheet-002", "sheet-003"]);

  act(() => canvasHarness.props?.sheetReorder?.onDrop());
  await waitFor(() =>
    expect(applyWithOutcome).toHaveBeenCalledWith({
      kind: "reorderSheet",
      sheetId: "sheet-001",
      targetIndex: 1,
    }, expect.any(Function)),
  );
  expect(applyWithOutcome).toHaveBeenCalledOnce();
});

test("blocks a newer reorder while a queued reorder is pending and restores the canonical session on failure", async () => {
  const visibleProjection = createThreeSheetProjection();
  const restoredProjection = structuredClone(visibleProjection);
  restoredProjection.state.revision += 1;
  restoredProjection.state.album.sheets.forEach((sheet, index) => {
    sheet.number = index + 2;
    sheet.role = index === restoredProjection.state.album.sheets.length - 1
      ? "final"
      : "internal";
  });
  restoredProjection.composition.sheets.forEach((sheet, index) => {
    sheet.number = index + 2;
  });
  restoredProjection.state.album.sheets.unshift({
    ...structuredClone(restoredProjection.state.album.sheets[0]),
    id: "sheet-restored",
    number: 1,
    role: "initial",
    activeSides: "right",
    pageNumbers: [1],
    frames: [],
  });
  restoredProjection.composition.sheets.unshift({
    ...structuredClone(restoredProjection.composition.sheets[0]),
    sheetId: "sheet-restored",
    number: 1,
    activeSides: "right",
    frames: [],
  });

  const pendingUndo = deferredProjection();
  const pendingReorder = deferredValue<
    Awaited<ReturnType<ProjectCorePort["applyWithOutcome"]>>
  >();
  const port = projectCorePortWithApply(async () => visibleProjection);
  port.undo = () => pendingUndo.promise;
  port.applyWithOutcome = vi.fn(() => pendingReorder.promise);
  let view!: ReturnType<typeof render>;
  const renderWorkspace = (currentProjection: EditorProjection) => (
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={currentProjection}
      projectCorePort={port}
      onProjectionChange={(nextProjection) => {
        view.rerender(renderWorkspace(nextProjection));
      }}
    />
  );
  view = render(renderWorkspace(visibleProjection));

  fireEvent.click(getApplicationCommand("Editar", "Desfazer"));
  performGridPointerReorder(view, 1, 0);
  finishGridPointerReorder(1, 0);

  expect(port.applyWithOutcome).not.toHaveBeenCalled();
  await act(async () => {
    pendingUndo.resolve(restoredProjection);
    await pendingUndo.promise;
  });
  await waitFor(() => expect(port.applyWithOutcome).toHaveBeenCalledOnce());
  await waitFor(() =>
    expect(
      Array.from(
        view.container.querySelectorAll<HTMLElement>(".sheet-grid-slot"),
        (slot) => slot.dataset.sheetId,
      ),
    ).toEqual(["sheet-restored", "sheet-001", "sheet-002", "sheet-003"]),
  );
  act(() => canvasHarness.props?.sheetReorder?.onPreview("sheet-003", 2));
  expect(canvasHarness.props?.sheetReorder).toMatchObject({
    disabled: true,
    status: "idle",
    representation: {
      order: ["sheet-restored", "sheet-001", "sheet-002", "sheet-003"],
      placeholderIndex: null,
      ghost: null,
    },
  });
  act(() => canvasHarness.props?.sheetReorder?.onDrop());
  expect(port.applyWithOutcome).toHaveBeenCalledOnce();

  await act(async () => {
    pendingReorder.reject(new Error("A ordem mudou antes do commit."));
    await pendingReorder.promise.catch(() => undefined);
  });

  await waitFor(() => {
    expect(
      Array.from(
        view.container.querySelectorAll<HTMLElement>(".sheet-grid-slot"),
        (slot) => slot.dataset.sheetId,
      ),
    ).toEqual(["sheet-restored", "sheet-001", "sheet-002", "sheet-003"]);
    expect(canvasHarness.props?.sheetReorder?.representation.order).toEqual([
      "sheet-restored",
      "sheet-001",
      "sheet-002",
      "sheet-003",
    ]);
    expect(canvasHarness.props?.sheetReorder?.status).toBe("idle");
    expect(canvasHarness.props?.sheetReorder?.disabled).toBe(false);
  });
});

test("keeps both Sheet reorder surfaces inert while a commit is pending", async () => {
  const physicalProjection = createThreeSheetProjection();
  const pendingReorder = deferredValue<
    Awaited<ReturnType<ProjectCorePort["applyWithOutcome"]>>
  >();
  const port = projectCorePortWithApply(async () => physicalProjection);
  port.applyWithOutcome = vi.fn(() => pendingReorder.promise);
  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={physicalProjection}
      projectCorePort={port}
      onProjectionChange={() => undefined}
    />,
  );

  performGridPointerReorder(view, 0, 1);
  finishGridPointerReorder(0, 1);

  await waitFor(() => expect(port.applyWithOutcome).toHaveBeenCalledOnce());
  expect(canvasHarness.props?.sheetReorder?.status).toBe("committing");
  for (const slot of view.container.querySelectorAll(".sheet-grid-slot")) {
    expect(slot).not.toHaveAttribute("draggable");
    expect(slot).not.toHaveAttribute("data-reorder-enabled");
  }

  await act(async () => {
    pendingReorder.reject(new Error("Falha controlada após observar o commit."));
    await pendingReorder.promise.catch(() => undefined);
  });
  await waitFor(() =>
    expect(screen.getByTestId("sheet-reorder-grid")).toHaveAttribute(
      "data-reorder-state",
      "idle",
    ),
  );
});

test.each(["bar", "grid"] as const)(
  "cancels and blocks the %s reorder preview while another structural commit is pending",
  async (surface) => {
    const physicalProjection = createThreeSheetProjection();
    const pendingConversion = deferredValue<
      Awaited<ReturnType<ProjectCorePort["applyWithOutcome"]>>
    >();
    const port = projectCorePortWithApply(async () => physicalProjection);
    port.applyWithOutcome = vi.fn(() => pendingConversion.promise);
    const view = render(
      <ProjectWorkspace
        exportPipelinePort={exportPipelinePort}
        projection={physicalProjection}
        projectCorePort={port}
        onProjectionChange={() => undefined}
      />,
    );

    act(() => canvasHarness.props?.onCenteredSheetChange?.("sheet-003"));
    const slots = Array.from(
      view.container.querySelectorAll<HTMLElement>(".sheet-grid-slot"),
    );
    if (surface === "bar") {
      act(() => canvasHarness.props?.sheetReorder?.onPreview("sheet-001", 1));
    } else {
      performGridPointerReorder(view, 0, 1);
    }
    expect(canvasHarness.props?.sheetReorder?.status).toBe("preview");

    fireEvent.click(
      getApplicationCommand("Lâmina", "Converter extremidade"),
    );
    await waitFor(() => expect(port.applyWithOutcome).toHaveBeenCalledOnce());

    expect(canvasHarness.props?.sheetReorder?.disabled).toBe(true);
    expect(canvasHarness.props?.sheetReorder?.status).toBe("idle");
    expect(screen.getByTestId("sheet-reorder-grid")).toHaveAttribute(
      "data-reorder-state",
      "idle",
    );
    for (const slot of slots) {
      expect(slot).not.toHaveAttribute("draggable");
      expect(slot).not.toHaveAttribute("data-reorder-enabled");
    }

    if (surface === "bar") {
      act(() => canvasHarness.props?.sheetReorder?.onPreview("sheet-001", 1));
      act(() => canvasHarness.props?.sheetReorder?.onDrop());
    } else {
      performGridPointerReorder(view, 0, 1);
      finishGridPointerReorder(0, 1);
    }
    expect(canvasHarness.props?.sheetReorder?.status).toBe("idle");
    expect(port.applyWithOutcome).toHaveBeenCalledOnce();

    await act(async () => {
      pendingConversion.reject(
        new Error("Falha estrutural controlada após observar a trava."),
      );
      await pendingConversion.promise.catch(() => undefined);
    });
  },
);

test("derives the Canvas technical guides from the canonical document", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(canvasHarness.props?.technicalGuides).toEqual({
    bleedUm: projection.state.document.bleedUm,
    safetyUm: projection.state.document.safetyUm,
  });
});

test("temporarily compacts the image panel during Sheet Edit Mode and restores its normal height", () => {
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );
  const workspace = view.container.querySelector(
    ".workspace-grid",
  ) as HTMLElement;
  const normalHeight = workspace.style.getPropertyValue(
    "--media-panel-height",
  );

  act(() => canvasHarness.props?.onEditSheet?.("sheet-001"));

  expect(canvasHarness.props?.mode).toEqual({
    kind: "sheet-editing",
    sheetId: "sheet-001",
  });
  expect(
    workspace.style.getPropertyValue("--media-panel-height"),
  ).toBe("120px");
  expect(
    screen.getByRole("separator", {
      name: "Redimensionar Painel de imagens",
    }),
  ).toHaveAttribute("aria-disabled", "true");

  const input = document.createElement("input");
  document.body.append(input);
  fireEvent.keyDown(input, { key: "Escape" });

  expect(canvasHarness.props?.mode).toEqual({ kind: "normal" });
  expect(
    workspace.style.getPropertyValue("--media-panel-height"),
  ).toBe(normalHeight);
  input.remove();
});

test("hydrates and publishes machine-local Inspector and media density preferences", async () => {
  let persisted = createWorkspacePreferences({
    inspectorSections: { "album.information": false },
    mediaThumbnailSize: 124,
  });
  const update = vi.fn<WorkspacePreferencesPort["update"]>(async (change) => {
    persisted = applyWorkspacePreferenceChange(persisted, change);
    return persisted;
  });
  const workspacePreferencesPort: WorkspacePreferencesPort = {
    load: vi.fn(async () => persisted),
    update,
  };
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      workspacePreferences={{
        kind: "persistent",
        port: workspacePreferencesPort,
      }}
      onProjectionChange={() => undefined}
    />,
  );

  const information = screen.getByRole("button", {
    name: "Informações do Álbum",
  });
  await waitFor(() =>
    expect(information).toHaveAttribute("aria-expanded", "false"),
  );
  fireEvent.click(information);
  await waitFor(() =>
    expect(update).toHaveBeenCalledWith({
      kind: "inspectorSection",
      preferenceKey: "album.information",
      open: true,
    }),
  );

  fireEvent.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  const size = screen.getByRole("slider", {
    name: "Tamanho das miniaturas",
  });
  expect(size).toHaveValue("124");
  fireEvent.change(size, { target: { value: "126" } });
  await waitFor(() =>
    expect(update).toHaveBeenCalledWith({
      kind: "mediaThumbnailSize",
      size: 126,
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));
  expect(screen.getByRole("group", { name: "Grade de Decorativos" })).toHaveStyle({
    "--media-thumbnail-size": "126px",
  });
  expect((await workspacePreferencesPort.load()).mediaThumbnailSize).toBe(126);
});

test("saving a custom Layout works from Sheet Design and the Edit menu while a Frame owns the Inspector", async () => {
  const sample = layoutPanelCorpus.cases["custom-save"];
  const initial = sample.before.projection;
  const sheet = initial.state.album.sheets[0];
  const port = projectCorePortWithApply(async () => initial);
  port.saveCustomLayout = vi.fn(async () => sample.saveResult!);
  const onProjectionChange = vi.fn();
  render(<ProjectWorkspace projection={initial} projectCorePort={port} onProjectionChange={onProjectionChange} />);
  act(() => canvasHarness.props?.onEditSheet?.(sheet.id));
  const save = screen.getByRole("button", { name: "Salvar disposição como Layout" });
  expect(save).toBeEnabled();
  await act(async () => fireEvent.click(save));
  expect(port.saveCustomLayout).toHaveBeenCalledExactlyOnceWith(sheet.id);
  expect(screen.getByText("Layout salvo em Personalizados.")).toBeInTheDocument();
  expect(canvasHarness.props?.mode).toEqual({ kind: "sheet-editing", sheetId: sheet.id });
  act(() => useEditorView.getState().selectFrames([sheet.frames[0].id]));
  expect(screen.queryByText("Layout salvo em Personalizados.")).not.toBeInTheDocument();
  const menu = getApplicationCommand("Editar", "Salvar disposição como Layout");
  expect(menu).toBeEnabled();
  await act(async () => fireEvent.click(menu));
  expect(port.saveCustomLayout).toHaveBeenCalledTimes(2);
  expect(screen.getByText("Layout salvo em Personalizados.")).toBeInTheDocument();
  expect(onProjectionChange).not.toHaveBeenCalled();
  expect(canvasHarness.props?.mode).toEqual({ kind: "sheet-editing", sheetId: sheet.id });
});

test.each(["exit", "collapse"])("save feedback disappears permanently when leaving its button: %s", async (transition) => {
  const sample = layoutPanelCorpus.cases["custom-save"];
  const initial = sample.before.projection;
  const sheetId = initial.state.album.sheets[0].id;
  const port = projectCorePortWithApply(async () => initial);
  port.saveCustomLayout = vi.fn(async () => sample.saveResult!);
  render(<ProjectWorkspace projection={initial} projectCorePort={port} onProjectionChange={() => undefined} />);
  act(() => canvasHarness.props?.onEditSheet?.(sheetId));
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "Salvar disposição como Layout" })));
  expect(screen.getByText("Layout salvo em Personalizados.")).toBeInTheDocument();
  await act(async () => {
    if (transition === "exit") useEditorView.getState().exitSheetEdit();
    else fireEvent.click(screen.getByRole("button", { name: "Design da Lâmina" }));
  });
  expect(screen.queryByText("Layout salvo em Personalizados.")).not.toBeInTheDocument();
  await act(async () => {
    if (transition === "exit") canvasHarness.props?.onEditSheet?.(sheetId);
    else fireEvent.click(screen.getByRole("button", { name: "Design da Lâmina" }));
  });
  expect(screen.queryByText("Layout salvo em Personalizados.")).not.toBeInTheDocument();
});

test("an empty edited Sheet explains why custom Layout saving is unavailable in both surfaces", () => {
  const initial = layoutPanelCorpus.cases.empty.before.projection;
  render(<ProjectWorkspace projection={initial} onProjectionChange={() => undefined} />);
  act(() => canvasHarness.props?.onEditSheet?.(initial.state.album.sheets[0].id));
  const save = screen.getByRole("button", { name: "Salvar disposição como Layout" });
  expect(save).toBeDisabled();
  expect(save).toHaveAttribute("title", "Adicione ao menos um Frame para salvar um Layout.");
  expect(getApplicationCommand("Editar", "Salvar disposição como Layout")).toBeDisabled();
});

test("Layout focus temporarily hides media and restores the mounted panel with its search", async () => {
  const sample = layoutPanelCorpus.cases.mixed.before;
  const sheetId = sample.projection.state.album.sheets[0].id;
  const port = projectCorePortWithApply(async () => sample.projection);
  port.queryLayouts = async (target) => sample.queries[target].query;
  port.previewLayout = async (selection) => sample.queries[sheetId].previews[selection.candidateIndex];
  render(<ProjectWorkspace projection={sample.projection} projectCorePort={port} onProjectionChange={() => undefined} />);
  const media = screen.getByRole("region", { name: "Painel de imagens" });
  const search = within(media).getByRole("searchbox", { name: "Buscar Fotos" });
  const initialHeight = (document.querySelector(".workspace-grid") as HTMLElement).style.getPropertyValue("--media-panel-height");
  fireEvent.change(search, { target: { value: "Minha busca" } });
  act(() => canvasHarness.props?.sheetLayouts?.onToggle(sheetId));
  await screen.findByRole("region", { name: "Painel de Layouts" });
  expect(canvasHarness.props?.mode).toEqual({ kind: "normal", isolatedSheetId: sheetId });
  expect(media).not.toBeVisible();
  expect(screen.queryByRole("separator", { name: "Redimensionar Painel de imagens" })).not.toBeInTheDocument();
  expect(document.querySelector(".workspace-grid")).toHaveStyle({ "--media-panel-height": "0px", "--media-splitter-size": "0px" });
  fireEvent.pointerDown(screen.getByTestId("album-canvas"));
  expect(screen.queryByRole("region", { name: "Painel de Layouts" })).not.toBeInTheDocument();
  expect(canvasHarness.props?.mode).toEqual({ kind: "normal" });
  expect(screen.getByRole("region", { name: "Painel de imagens" })).toBe(media);
  expect(media).toBeVisible();
  expect(search).toHaveValue("Minha busca");
  expect(document.querySelector(".workspace-grid")).toHaveStyle({ "--media-panel-height": initialHeight });
});

test("closing Layouts keeps a previously hidden media panel hidden", async () => {
  const sample = layoutPanelCorpus.cases.mixed.before;
  const sheetId = sample.projection.state.album.sheets[0].id;
  const port = projectCorePortWithApply(async () => sample.projection);
  port.queryLayouts = async (target) => sample.queries[target].query;
  port.previewLayout = async (selection) => sample.queries[sheetId].previews[selection.candidateIndex];
  render(<ProjectWorkspace projection={sample.projection} projectCorePort={port} onProjectionChange={() => undefined} />);
  fireEvent.click(getApplicationCommand("Exibir", "Painel de imagens"));
  expect(screen.queryByRole("region", { name: "Painel de imagens" })).not.toBeInTheDocument();
  act(() => canvasHarness.props?.sheetLayouts?.onToggle(sheetId));
  await screen.findByRole("region", { name: "Painel de Layouts" });
  fireEvent.pointerDown(screen.getByTestId("album-canvas"));
  expect(screen.queryByRole("region", { name: "Painel de imagens" })).not.toBeInTheDocument();
  expect(document.querySelector(".workspace-grid")).toHaveStyle({ "--media-panel-height": "0px" });
});

test("toggles canonical panel commands and persists visibility with the current size", async () => {
  let persisted = createWorkspacePreferences({
    workspacePanels: {
      inspector: { size: 350, visible: false },
      media: { size: 200, visible: true },
    },
  });
  const update = vi.fn<WorkspacePreferencesPort["update"]>(async (change) => {
    persisted = applyWorkspacePreferenceChange(persisted, change);
    return persisted;
  });
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      workspacePreferences={{
        kind: "persistent",
        port: { load: async () => persisted, update },
      }}
      onProjectionChange={() => undefined}
    />,
  );

  await waitFor(() =>
    expect(
      screen.queryByRole("complementary", { name: "Painel contextual" }),
    ).not.toBeInTheDocument(),
  );
  const showInspector = getApplicationCommand(
    "Exibir",
    "Painel contextual",
  );
  expect(showInspector).toHaveAttribute("aria-checked", "false");
  fireEvent.click(showInspector);

  await waitFor(() =>
    expect(update).toHaveBeenCalledWith({
      kind: "workspacePanelVisibility",
      panel: "inspector",
      visible: true,
    }),
  );
  expect(
    screen.getByRole("complementary", { name: "Painel contextual" }),
  ).toBeInTheDocument();

  const hideMedia = getApplicationCommand("Exibir", "Painel de imagens");
  expect(hideMedia).toHaveAttribute("aria-checked", "true");
  fireEvent.click(hideMedia);

  await waitFor(() =>
    expect(update).toHaveBeenCalledWith({
      kind: "workspacePanelVisibility",
      panel: "media",
      visible: false,
    }),
  );
  expect(
    screen.queryByRole("region", { name: "Painel de imagens" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("region", { name: "Área de composição" }).parentElement,
  ).toHaveStyle({
    "--media-panel-height": "0px",
    "--media-splitter-size": "0px",
  });
});

test("consumes the first Escape in the image-panel options before leaving Sheet Edit Mode", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => canvasHarness.props?.onEditSheet?.("sheet-001"));
  expect(canvasHarness.props?.mode).toEqual({
    kind: "sheet-editing",
    sheetId: "sheet-001",
  });

  fireEvent.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  expect(
    screen.getByRole("group", { name: "Filtro, ordem e tamanho" }),
  ).toBeInTheDocument();

  expect(fireEvent.keyDown(document, { key: "Escape" })).toBe(false);

  expect(
    screen.queryByRole("group", { name: "Filtro, ordem e tamanho" }),
  ).not.toBeInTheDocument();
  expect(canvasHarness.props?.mode).toEqual({
    kind: "sheet-editing",
    sheetId: "sheet-001",
  });

  fireEvent.keyDown(window, { key: "Escape" });
  expect(canvasHarness.props?.mode).toEqual({ kind: "normal" });
});

test("starts the implemented Lâmina export from the Arquivo menu", () => {
  const startSheet = vi.fn<ExportPort["startSheet"]>(() => ({
    completion: Promise.resolve({
      status: "completed",
      result: { widthPx: 600, heightPx: 300 },
    }),
    cancel: async () => "not_found",
  }));
  render(
    <ProjectWorkspace
      exportPort={{ startSheet }}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(getApplicationCommand("Arquivo", "Exportar Lâmina…"));

  expect(startSheet).toHaveBeenCalledWith("sheet-001", expect.any(Function));
});

test("uses contextual empty states when the editor has no materialized content", () => {
  const emptyProjection = createEmptyProjection();
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={emptyProjection}
      projectSessionPort={projectSessionPortWithApply(async () =>
        emptyProjection
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const mediaEmptyState = screen.getByRole("status", {
    name: "Nenhuma Foto importada",
  });

  expect(mediaEmptyState).toHaveClass("ui-empty-state");
  expect(mediaEmptyState).toHaveClass("media-empty-state--catalog");
  expect(
    screen.getByRole("status", { name: "Nenhuma Lâmina na Grade" }),
  ).toHaveClass("ui-empty-state");
});

test("offers the three close choices for a native request and Cancel keeps the Project", async () => {
  const harness = projectWindowHarness();
  const dirtyProjection = {
    ...projection,
    state: {
      ...projection.state,
      dirty: true,
    },
  };
  harness.port.resolveClose = vi.fn(async () => ({
    kind: "cancelled" as const,
    projection: dirtyProjection,
  }));

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={dirtyProjection}
      projectCorePort={projectCorePortWithApply(async () =>
        dirtyProjection
      )}
      projectDialogPort={harness.dialog.port}
      projectWindowPort={harness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    expect(harness.emitCloseRequested()).toBeUndefined();
    expect(harness.dialog.present).toHaveBeenCalledWith({
      busy: false,
      kind: "projectCloseConfirmation",
    });
  });

  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(
    screen.getByRole("menuitem", { name: "Editar", hidden: true }),
  ).toBeDisabled();

  act(() => harness.dialog.emit("cancelProjectClose"));

  await waitFor(() => {
    expect(harness.port.resolveClose).toHaveBeenCalledWith("cancel");
    expect(harness.dialog.dismiss).toHaveBeenCalled();
  });
  expect(harness.dialog.present).not.toHaveBeenCalledWith({
    busy: true,
    kind: "projectCloseConfirmation",
  });
  expect(
    getApplicationCommand("Editar", "Desfazer"),
  ).toBeEnabled();
});

test("releases an application close when its confirmation window cannot be presented", async () => {
  const harness = projectWindowHarness();
  const restoredProjection = {
    ...projection,
    state: { ...projection.state, revision: projection.state.revision + 1 },
  };
  const onProjectionChange = vi.fn();
  harness.dialog.present.mockRejectedValueOnce(
    new Error("Não foi possível abrir a confirmação."),
  );
  harness.port.resolveClose = vi.fn(async () => ({
    kind: "cancelled" as const,
    projection: restoredProjection,
  }));

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectDialogPort={harness.dialog.port}
      projectWindowPort={harness.port}
      onProjectionChange={onProjectionChange}
    />,
  );

  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivo" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Fechar Projeto" }));

  await waitFor(() => {
    expect(harness.port.resolveClose).toHaveBeenCalledWith("cancel");
    expect(onProjectionChange).toHaveBeenCalledWith(restoredProjection);
    expect(harness.dialog.present).toHaveBeenLastCalledWith({
      kind: "projectOperationFailure",
      message: "Não foi possível abrir a confirmação.",
    });
  });
});

test("releases a native close when its confirmation window cannot be presented", async () => {
  const harness = projectWindowHarness();
  const onProjectionChange = vi.fn();
  harness.dialog.present.mockRejectedValueOnce(
    new Error("Não foi possível abrir a confirmação."),
  );
  harness.port.resolveClose = vi.fn(async () => ({
    kind: "cancelled" as const,
    projection,
  }));

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectDialogPort={harness.dialog.port}
      projectWindowPort={harness.port}
      onProjectionChange={onProjectionChange}
    />,
  );
  await waitFor(() => expect(harness.port.onCloseRequested).toHaveBeenCalled());

  act(() => harness.emitCloseRequested());

  await waitFor(() => {
    expect(harness.port.resolveClose).toHaveBeenCalledWith("cancel");
    expect(onProjectionChange).toHaveBeenCalledWith(projection);
    expect(harness.dialog.present).toHaveBeenLastCalledWith({
      kind: "projectOperationFailure",
      message: "Não foi possível abrir a confirmação.",
    });
  });
});

test("uses the same close decision for the application command and blocks it while resolving", async () => {
  const harness = projectWindowHarness();
  let finish!: () => void;
  harness.port.resolveClose = vi.fn(
    () =>
      new Promise<ProjectCloseResolution>((resolve) => {
        finish = () => resolve({ kind: "closed" });
      }),
  );

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={{
        ...projection,
        state: { ...projection.state, dirty: true },
      }}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectDialogPort={harness.dialog.port}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      projectWindowPort={harness.port}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivo" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Fechar Projeto" }));
  await waitFor(() =>
    expect(harness.port.requestClose).toHaveBeenCalledOnce(),
  );
  await waitFor(() =>
    expect(harness.dialog.present).toHaveBeenCalledWith({
      busy: false,
      kind: "projectCloseConfirmation",
    }),
  );

  act(() => harness.dialog.emit("saveAndClose"));
  expect(harness.port.resolveClose).toHaveBeenCalledWith("saveAndClose");
  expect(harness.dialog.present).toHaveBeenLastCalledWith({
    busy: true,
    kind: "projectCloseConfirmation",
  });
  expect(
    screen.getByRole("button", { name: "Exportar Lâmina", hidden: true }),
  ).toBeDisabled();

  await act(async () => finish());
  expect(
    screen.getByRole("menuitem", { name: "Editar" }),
  ).toBeDisabled();
});

test("sends Discard and resumes the unchanged Project after a conclusive save failure", async () => {
  const discardHarness = projectWindowHarness();
  const { unmount } = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectDialogPort={discardHarness.dialog.port}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      projectWindowPort={discardHarness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    discardHarness.emitCloseRequested();
    expect(discardHarness.dialog.present).toHaveBeenCalled();
  });
  act(() => discardHarness.dialog.emit("discardAndClose"));
  expect(discardHarness.port.resolveClose).toHaveBeenCalledWith(
    "discardAndClose",
  );
  unmount();

  const failureHarness = projectWindowHarness();
  failureHarness.port.resolveClose = vi.fn(async () => {
    throw new ProjectCloseError(
      "persisted_baseline_conflict",
      "O arquivo do Projeto foi alterado fora do MyAlbuns.",
    );
  });
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectDialogPort={failureHarness.dialog.port}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      projectWindowPort={failureHarness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    failureHarness.emitCloseRequested();
    expect(failureHarness.dialog.present).toHaveBeenCalled();
  });
  act(() => failureHarness.dialog.emit("saveAndClose"));

  await waitFor(() =>
    expect(failureHarness.dialog.present).toHaveBeenLastCalledWith({
      kind: "projectCloseFailure",
      message: "O arquivo do Projeto foi alterado fora do MyAlbuns.",
    }),
  );
  act(() => failureHarness.dialog.emit("dismissProjectCloseFailure"));
  await screen.findByRole("menuitem", { name: "Editar" });
  expect(
    getApplicationCommand("Editar", "Desfazer"),
  ).toBeEnabled();
});

test("never resumes or reports success after an indeterminate close save", async () => {
  const harness = projectWindowHarness();
  harness.port.resolveClose = vi.fn(async () => {
    throw new ProjectCloseError(
      "save_state_indeterminate",
      "Não foi possível confirmar qual revisão ficou no arquivo.",
    );
  });
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectDialogPort={harness.dialog.port}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      projectWindowPort={harness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    harness.emitCloseRequested();
    expect(harness.dialog.present).toHaveBeenCalled();
  });

  act(() => harness.dialog.emit("saveAndClose"));

  await waitFor(() =>
    expect(harness.dialog.present).toHaveBeenLastCalledWith({
      kind: "projectCloseFailure",
      message: "Não foi possível confirmar qual revisão ficou no arquivo.",
    }),
  );
  act(() => harness.dialog.emit("dismissProjectCloseFailure"));
  expect(
    screen.getByRole("menuitem", { name: "Editar" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Exportar Lâmina" }),
  ).toBeDisabled();
});

test("blocks only Project commands while its Export attempt is active", async () => {
  let emit!: (event: ExportProgressEvent) => void;
  let finish!: (outcome: ExportOutcome) => void;
  const completion = new Promise<ExportOutcome>((resolve) => {
    finish = resolve;
  });
  const controlledExportPipelinePort: ExportPipelinePort = {
    startSheet: (_sheetId, onEvent) => {
      emit = onEvent;
      return {
        completion,
        cancel: async () => "requested",
      };
    },
  };
  const projectSessionPort = projectSessionPortWithApply(async () => projection);
  const dialog = projectDialogHarness();
  projectSessionPort.undo = vi.fn(async () => projection);
  const projectCorePort = projectCorePortWithApply(async () => projection);
  projectCorePort.undo = vi.fn(async () => projection);

  render(
    <ProjectWorkspace
      exportPipelinePort={controlledExportPipelinePort}
      projection={projection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      projectCorePort={projectCorePort}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(
    screen.getByRole("button", { name: "Exportar Lâmina" }),
  );
  expect(screen.getByRole("menuitem", { name: "Editar" })).toBeDisabled();

  act(() => {
    emit({ event: "started", cancellable: true });
  });
  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  expect(projectCorePort.undo).not.toHaveBeenCalled();

  await act(async () => {
    finish({ status: "cancelled" });
    await completion;
  });

  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  expect(projectCorePort.undo).not.toHaveBeenCalled();

  act(() => dialog.emit("dismissExport"));
  expect(getApplicationCommand("Editar", "Desfazer")).toBeEnabled();
});

test("forwards a fatal Canvas graphics diagnostic without interpreting it", () => {
  const onGraphicsUnavailable = vi.fn();
  const diagnostic: GraphicsDiagnostic = {
    supported: false,
    code: "webgl2_unavailable",
    renderer: "indisponível",
    reason: "O Canvas real não possui WebGL2.",
    limits: null,
  };
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
      onGraphicsUnavailable={onGraphicsUnavailable}
    />,
  );

  act(() => {
    canvasHarness.props?.onGraphicsUnavailable?.(diagnostic);
  });

  expect(onGraphicsUnavailable).toHaveBeenCalledWith(diagnostic);
});

test("restores accordion preferences after context changes and remounts", async () => {
  const workspacePreferencesPort = createFallbackWorkspacePreferencesPort();
  const renderWorkspace = () =>
    render(
      <ProjectWorkspace
        exportPipelinePort={exportPipelinePort}
        projection={projection}
        projectSessionPort={projectSessionPortWithApply(async () => projection)}
        workspacePreferences={{
          kind: "persistent",
          port: workspacePreferencesPort,
        }}
        projectCorePort={projectCorePortWithApply(async () => projection)}
        onProjectionChange={() => undefined}
      />,
    );

  const firstView = renderWorkspace();
  const albumInformation = screen.getByRole("button", {
    name: "Informações do Álbum",
  });
  expect(albumInformation).toHaveAttribute("aria-expanded", "true");

  fireEvent.click(albumInformation);
  expect(albumInformation).toHaveAttribute("aria-expanded", "false");

  act(() => useEditorView.setState({ selectedFrameIds: ["frame-001"] }));
  expect(
    screen.getByRole("button", { name: "Design" }),
  ).toBeInTheDocument();

  act(() => useEditorView.setState({ selectedFrameIds: [] }));
  expect(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  ).toHaveAttribute("aria-expanded", "false");

  await waitFor(async () =>
    expect(
      (await workspacePreferencesPort.load()).inspectorSections[
        "album.information"
      ],
    ).toBe(false),
  );

  firstView.unmount();
  renderWorkspace();
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Informações do Álbum" }),
    ).toHaveAttribute("aria-expanded", "false"),
  );
});

test("uses the reference chrome and collapsible contextual sections", () => {
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(screen.getByLabelText("MyAlbuns")).toBeInTheDocument();
  expect(
    screen.getByText("Álbum Horizonte", {
      selector: ".ui-application-header__identity strong",
    }),
  ).toBeInTheDocument();
  expect(screen.getByText("300×300 mm · 1 Lâmina")).toBeInTheDocument();
  expect(screen.queryByText("Intel(R) UHD Graphics")).not.toBeInTheDocument();
  expect(screen.queryByText("revisão 25")).not.toBeInTheDocument();
  expect(screen.queryByText("3 Fotos vinculadas")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("Zoom do Canvas")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Geral" })).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Grade de Lâminas" }),
  ).toBeInTheDocument();
});

test("shows the physical configuration projected from the opened Project", () => {
  const configuredProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      document: {
        displayUnit: "cm" as const,
        sheetWidthUm: 508_000,
        sheetHeightUm: 254_000,
        dpi: 240,
        bleedUm: 2_500,
        safetyUm: 5_000,
      },
      album: {
        ...projection.state.album,
        sheets: projection.state.album.sheets.map((sheet) => ({
          ...sheet,
          activeSides: "both",
        })),
      },
    },
  };

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={configuredProjection}
      projectCorePort={projectCorePortWithApply(
        async () => configuredProjection,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const albumInformation = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );

  const sheetDimensions = within(
    albumInformation.getByRole("group", { name: "Dimensão da Lâmina" }),
  );
  expect(sheetDimensions.getByRole("textbox", { name: "Largura" })).toHaveValue(
    "50.8",
  );
  expect(sheetDimensions.getByRole("textbox", { name: "Altura" })).toHaveValue(
    "25.4",
  );

  const pageDimensions = within(
    albumInformation.getByRole("group", { name: "Dimensão da Página" }),
  );
  expect(pageDimensions.getByLabelText("Largura")).toHaveTextContent(
    "25.4 cm",
  );
  expect(pageDimensions.getByLabelText("Altura")).toHaveTextContent(
    "25.4 cm",
  );
  expect(pageDimensions.getByLabelText("Largura")).toHaveClass(
    "inspector-readout--integrated",
  );
  expect(albumInformation.getByLabelText("DPI")).toHaveValue("240");
  expect(albumInformation.getByRole("textbox", { name: "Sangria" })).toHaveValue(
    "0.25",
  );
  expect(
    albumInformation.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("0.5");
});

test("projects the pending Unidade across the Project Window without changing Album Design", async () => {
  const projectionWithBorder: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      dirty: false,
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          frameBorder: { kind: "solid", rgb: "#2C2924", widthUm: 2_540 },
        },
      },
    },
    composition: {
      ...projection.composition,
      frameBorder: { kind: "solid", rgb: "#2C2924", widthUm: 2_540 },
    },
  };
  const apply = vi.fn(async () => projectionWithBorder);
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithBorder}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const informationSection = screen
    .getByRole("button", { name: "Informações do Álbum" })
    .closest("section") as HTMLElement;
  const designSection = screen
    .getByRole("button", { name: "Design do Álbum" })
    .closest("section") as HTMLElement;
  const design = within(designSection);
  const designApply = design.getByRole("button", { name: "Aplicar" });

  fireEvent.change(within(informationSection).getByLabelText("Unidade"), {
    target: { value: "in" },
  });

  expect(await screen.findByText("11.811×11.811 pol · 1 Lâmina")).toBeVisible();
  expect(
    design.getByText("Borda padrão").closest("label"),
  ).toHaveTextContent("0.1 pol");
  expect(
    design.getByText("Espaço entre Frames").closest("label"),
  ).toHaveTextContent("0.236 pol");
  expect(designApply).toBeDisabled();
  expect(screen.getByText("salvo")).toBeVisible();
  expect(apply).not.toHaveBeenCalled();

  fireEvent.click(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  );
  expect(await screen.findByText("300×300 mm · 1 Lâmina")).toBeVisible();
  expect(
    design.getByText("Espaço entre Frames").closest("label"),
  ).toHaveTextContent("6 mm");
  expect(designApply).toBeDisabled();

  fireEvent.click(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  );
  fireEvent.change(
    within(
      screen
        .getByRole("button", { name: "Informações do Álbum" })
        .closest("section") as HTMLElement,
    ).getByLabelText("Unidade"),
    { target: { value: "in" } },
  );
  expect(await screen.findByText("11.811×11.811 pol · 1 Lâmina")).toBeVisible();

  const otherProject: EditorProjection = {
    ...projectionWithBorder,
    state: {
      ...projectionWithBorder.state,
      projectId: "project-spike-002",
      document: {
        ...projectionWithBorder.state.document,
        displayUnit: "cm",
      },
    },
  };
  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={otherProject}
      projectSessionPort={projectSessionPortWithApply(async () => otherProject)}
      onProjectionChange={() => undefined}
    />,
  );
  expect(screen.getByText("30×30 cm · 1 Lâmina")).toBeVisible();
  expect(apply).not.toHaveBeenCalled();
});

test("clears pending Apply actions when their inspector forms are collapsed", async () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  const informationTrigger = screen.getByRole("button", {
    name: "Informações do Álbum",
  });
  const designTrigger = screen.getByRole("button", {
    name: "Design do Álbum",
  });
  const information = within(informationTrigger.closest("section") as HTMLElement);
  const design = within(designTrigger.closest("section") as HTMLElement);
  const informationApply = information.getByRole("button", { name: "Aplicar" });
  const designApply = design.getByRole("button", { name: "Aplicar" });

  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  fireEvent.change(design.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });

  await waitFor(() => expect(informationApply).toBeEnabled());
  await waitFor(() => expect(designApply).toBeEnabled());

  fireEvent.click(informationTrigger);
  fireEvent.click(designTrigger);

  await waitFor(() => expect(informationApply).toBeDisabled());
  await waitFor(() => expect(designApply).toBeDisabled());
});

test("uses the current reference layout for the Album context", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  const albumInformationTrigger = screen.getByRole("button", {
    name: "Informações do Álbum",
  });
  const albumInformationSection = albumInformationTrigger
    .closest("section") as HTMLElement;
  const albumDesignTrigger = screen.getByRole("button", {
    name: "Design do Álbum",
  });
  const albumDesignSection = albumDesignTrigger
    .closest("section") as HTMLElement;
  const albumInformation = within(albumInformationSection);
  const albumDesign = within(albumDesignSection);
  const albumInformationApply = albumInformation.getByRole("button", {
    name: "Aplicar",
  });
  const albumDesignApply = albumDesign.getByRole("button", {
    name: "Aplicar",
  });

  expect(albumInformation.queryByText("Projeto")).not.toBeInTheDocument();
  expect(albumInformation.queryByText("Verificação")).not.toBeInTheDocument();
  expect(
    albumInformation.queryByLabelText("Nome do Projeto"),
  ).not.toBeInTheDocument();
  expect(
    albumInformation.queryByText("Frames placeholder"),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Design do Álbum" }),
  ).toHaveAttribute("aria-expanded", "true");
  const albumDesignPreview = albumDesign.getByLabelText(
    "Prévia do padrão visual do Álbum",
  );
  expect(albumDesignPreview).toBeInTheDocument();
  expect(
    within(albumDesignPreview).getByRole("img", {
      name: "Composição do padrão visual do Álbum",
    }),
  ).toBeInTheDocument();
  expect(
    within(albumDesignPreview).queryByLabelText("Guias técnicas da Lâmina"),
  ).not.toBeInTheDocument();
  expect(
    albumDesignPreview.querySelector(".visual-preview-fixed-selection"),
  ).not.toBeInTheDocument();
  expect(
    within(albumDesignPreview).getByRole("group", {
      name: "Escopo do padrão visual do Álbum",
    }),
  ).toBeInTheDocument();
  expect(
    albumInformationSection.querySelector(
      '[data-placeholder-feature="album-end-sheet-settings"]',
    ),
  ).not.toBeInTheDocument();
  expect(
    albumInformationSection.querySelector(
      '[data-placeholder-feature="album-technical-area-settings"]',
    ),
  ).not.toBeInTheDocument();
  const compactControls = albumInformationSection.querySelector(
    ".document-compact-controls",
  ) as HTMLElement;
  expect(within(compactControls).getByLabelText("Unidade")).toBeInTheDocument();
  expect(within(compactControls).getByLabelText("DPI")).toBeInTheDocument();
  expect(albumInformationApply).toBeDisabled();
  expect(albumDesignApply).toBeDisabled();
  expect(albumInformationApply.closest(".inspector-section-header")).toContainElement(
    albumInformationTrigger,
  );
  expect(albumDesignApply.closest(".inspector-section-header")).toContainElement(
    albumDesignTrigger,
  );
  expect(albumDesignApply).not.toHaveAttribute("data-placeholder-feature");
  expect(albumInformation.getByText("Estrutura")).toBeInTheDocument();
  expect(albumInformation.getByText("Documento")).toBeInTheDocument();
  expect(albumInformation.getByText("Áreas técnicas")).toBeInTheDocument();
  expect(albumDesign.queryByText("Estrutura")).not.toBeInTheDocument();
  expect(albumDesign.queryByText("Documento")).not.toBeInTheDocument();
  expect(albumDesign.queryByText("Áreas técnicas")).not.toBeInTheDocument();
  expect(albumDesign.getByText("Padrões visuais")).toBeInTheDocument();
  expect(albumDesign.getByText("Padrão dos Frames")).toBeInTheDocument();
  expect(
    albumDesign.getByRole("slider", { name: "Espessura da Borda" }),
  ).toBeInTheDocument();
  expect(
    albumDesign.queryByRole("checkbox", { name: "Exibir borda" }),
  ).not.toBeInTheDocument();
  expect(albumDesign.getByLabelText("Cor do Background")).toBeInTheDocument();
});

test("edits and applies the complete Album design draft as one intent", async () => {
  const editableDesignProjection = {
    ...decorativeProjection,
    state: {
      ...decorativeProjection.state,
      album: {
        ...decorativeProjection.state.album,
        visualDefaults: {
          ...decorativeProjection.state.album.visualDefaults,
          overlay: { scope: "bothSides" as const, both: null },
        },
      },
    },
    composition: {
      ...decorativeProjection.composition,
      sheets: decorativeProjection.composition.sheets.map((sheet) => ({
        ...sheet,
        overlays: [],
      })),
    },
  } satisfies EditorProjection;
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () =>
    editableDesignProjection,
  );

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={editableDesignProjection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  const applyDesign = albumDesign.getByRole("button", { name: "Aplicar" });
  expect(applyDesign).toBeDisabled();

  const scopeControls = within(
    albumDesign.getByLabelText("Prévia do padrão visual do Álbum"),
  ).getByRole("group", { name: "Escopo do padrão visual do Álbum" });
  expect(within(scopeControls).getAllByRole("button")).toHaveLength(3);
  expect(
    within(scopeControls).getByRole("button", { name: "Ambos os lados" }),
  ).toHaveAttribute("aria-pressed", "true");

  fireEvent.click(albumDesign.getByRole("button", { name: "Lado esquerdo" }));
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  expect(
    albumDesign.queryByRole("button", { name: "Escolher Overlay" }),
  ).not.toBeInTheDocument();
  expect(
    albumDesign.queryByRole("button", { name: /Abrir mais opções/ }),
  ).not.toBeInTheDocument();
  expect(
    albumDesign.getByRole("group", { name: "Opções de Background" }),
  ).toBeInTheDocument();

  fireEvent.click(
    albumDesign.getByRole("button", {
      name: "Escolher Decorativo para Overlay",
    }),
  );
  fireEvent.click(
    albumDesign.getByRole("menuitem", {
      name: "Usar Overlay Overlay translúcido.png",
    }),
  );
  expect(
    albumDesign.queryByRole("menu", { name: "Decorativos para Overlay" }),
  ).not.toBeInTheDocument();
  expect(albumDesign.queryByRole("dialog")).not.toBeInTheDocument();
  const frame = albumDesign.getByLabelText("Frame demonstrativo esquerdo 1");
  const overlay = albumDesign.getByLabelText("Overlay do lado esquerdo");
  expect(
    frame.compareDocumentPosition(overlay) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  fireEvent.change(albumDesign.getByLabelText("Cor da Borda"), {
    target: { value: "#2c2924" },
  });
  const borderWidth = albumDesign.getByRole("slider", {
    name: "Espessura da Borda",
  });
  expect(borderWidth).toHaveAttribute("min", "0");
  fireEvent.change(borderWidth, { target: { value: "1250" } });

  expect(applyDesign).toBeEnabled();
  fireEvent.click(applyDesign);

  await waitFor(() =>
    expect(apply).toHaveBeenCalledWith({
      kind: "setVisualDefaults",
      visualDefaults: {
        background: {
          scope: "perSide",
          left: { kind: "color", rgb: "#F7F5F0" },
          right: { kind: "color", rgb: "#FFFFFF" },
        },
        overlay: {
          scope: "perSide",
          left: { kind: "media", mediaId: "decorative-overlay" },
          right: null,
        },
        frameBorder: {
          kind: "solid",
          rgb: "#2C2924",
          widthUm: 1_250,
        },
      },
    }, expect.any(Function)),
  );
});

test("prevents re-entering Album Design Apply while its mutation is pending", async () => {
  const pending = deferredProjection();
  const apply = vi.fn<ProjectSessionPort["apply"]>(() => pending.promise);

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  const applyDesign = albumDesign.getByRole("button", { name: "Aplicar" });
  expect(applyDesign).toBeEnabled();

  fireEvent.click(applyDesign);
  fireEvent.click(applyDesign);

  expect(applyDesign).toBeDisabled();
  expect(apply).toHaveBeenCalledOnce();

  await act(async () => {
    pending.resolve(projection);
    await pending.promise;
  });
  await waitFor(() => expect(apply).toHaveBeenCalledOnce());
});

test("saves the revision committed by a pending Album Design Apply", async () => {
  const pendingApply = deferredProjection();
  const appliedProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      dirty: true,
      revision: projection.state.revision + 1,
    },
  };
  const savedProjection: EditorProjection = {
    ...appliedProjection,
    state: {
      ...appliedProjection.state,
      dirty: false,
      savedRevision: appliedProjection.state.revision,
    },
  };
  const projectSessionPort = projectSessionPortWithApply(
    vi.fn(() => pendingApply.promise),
  );
  const save = vi.fn<ProjectSessionPort["save"]>(async () => ({
    outcome: {
      kind: "saved",
      revision: savedProjection.state.revision,
    },
    projection: savedProjection,
  }));
  projectSessionPort.save = save;

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.click(albumDesign.getByRole("button", { name: "Aplicar" }));
  fireEvent.click(getApplicationCommand("Arquivo", "Salvar"));

  expect(save).not.toHaveBeenCalled();

  await act(async () => {
    pendingApply.resolve(appliedProjection);
    await pendingApply.promise;
  });

  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(appliedProjection.state.revision),
  );
});

test("cancels a queued Save when Album Design Apply fails and allows a clean retry", async () => {
  const pendingApply = deferredProjection();
  const dialog = projectDialogHarness();
  const failure = new Error("O Design do Álbum não pôde ser aplicado.");
  const appliedProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      dirty: true,
      revision: projection.state.revision + 1,
    },
  };
  const savedProjection: EditorProjection = {
    ...appliedProjection,
    state: {
      ...appliedProjection.state,
      dirty: false,
      savedRevision: appliedProjection.state.revision,
    },
  };
  const apply = vi
    .fn<ProjectSessionPort["apply"]>()
    .mockImplementationOnce(() => pendingApply.promise)
    .mockResolvedValueOnce(appliedProjection);
  const save = vi.fn<ProjectSessionPort["save"]>(async () => ({
    outcome: {
      kind: "saved",
      revision: savedProjection.state.revision,
    },
    projection: savedProjection,
  }));
  const projectSessionPort = projectSessionPortWithApply(apply);
  projectSessionPort.save = save;

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.click(albumDesign.getByRole("button", { name: "Aplicar" }));
  fireEvent.click(getApplicationCommand("Arquivo", "Salvar"));

  await act(async () => {
    pendingApply.reject(failure);
    await pendingApply.promise.catch(() => undefined);
  });

  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "projectOperationFailure",
      message: failure.message,
    }),
  );
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(save).not.toHaveBeenCalled();
  await waitFor(() =>
    expect(albumDesign.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  const dismissCount = dialog.dismiss.mock.calls.length;
  act(() => dialog.emit("dismissProjectOperationFailure"));
  await waitFor(() =>
    expect(dialog.dismiss).toHaveBeenCalledTimes(dismissCount + 1),
  );

  fireEvent.click(albumDesign.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(apply).toHaveBeenCalledTimes(2));
  fireEvent.click(getApplicationCommand("Arquivo", "Salvar"));

  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(appliedProjection.state.revision),
  );
});

test("clears pending Save state after a queued Album Design save fails", async () => {
  const pendingApply = deferredProjection();
  const dialog = projectDialogHarness();
  const appliedProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      dirty: true,
      revision: projection.state.revision + 1,
    },
  };
  const savedProjection: EditorProjection = {
    ...appliedProjection,
    state: {
      ...appliedProjection.state,
      dirty: false,
      savedRevision: appliedProjection.state.revision,
    },
  };
  const saveFailure = new SaveProjectError(
    "persisted_baseline_conflict",
    "O arquivo do Projeto foi alterado fora do MyAlbuns.",
  );
  const save = vi
    .fn<ProjectSessionPort["save"]>()
    .mockRejectedValueOnce(saveFailure)
    .mockResolvedValueOnce({
      outcome: {
        kind: "saved",
        revision: savedProjection.state.revision,
      },
      projection: savedProjection,
    });
  const projectSessionPort = projectSessionPortWithApply(
    () => pendingApply.promise,
  );
  projectSessionPort.save = save;

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.click(albumDesign.getByRole("button", { name: "Aplicar" }));
  fireEvent.click(getApplicationCommand("Arquivo", "Salvar"));

  await act(async () => {
    pendingApply.resolve(appliedProjection);
    await pendingApply.promise;
  });

  await waitFor(() => expect(save).toHaveBeenCalledOnce());
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "projectOperationFailure",
      message: saveFailure.message,
    }),
  );
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeEnabled(),
  );
  const dismissCount = dialog.dismiss.mock.calls.length;
  act(() => dialog.emit("dismissProjectOperationFailure"));
  await waitFor(() =>
    expect(dialog.dismiss).toHaveBeenCalledTimes(dismissCount + 1),
  );
  fireEvent.click(getApplicationCommand("Arquivo", "Salvar"));

  await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
  expect(save).toHaveBeenNthCalledWith(
    2,
    appliedProjection.state.revision,
  );
}, 15_000);

test("revalidates queued Redo after Album Design Apply changes History eligibility", async () => {
  const pendingApply = deferredProjection();
  const projectionWithRedo: EditorProjection = {
    ...projection,
    state: { ...projection.state, canRedo: true },
  };
  const appliedProjection: EditorProjection = {
    ...projectionWithRedo,
    state: {
      ...projectionWithRedo.state,
      canRedo: false,
      revision: projectionWithRedo.state.revision + 1,
    },
  };
  const projectSessionPort = projectSessionPortWithApply(
    () => pendingApply.promise,
  );
  const redo = vi.fn<ProjectSessionPort["redo"]>(async () => projection);
  projectSessionPort.redo = redo;
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithRedo}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.click(albumDesign.getByRole("button", { name: "Aplicar" }));
  fireEvent.keyDown(window, { ctrlKey: true, key: "y" });

  await act(async () => {
    pendingApply.resolve(appliedProjection);
    await pendingApply.promise;
  });

  await waitFor(() =>
    expect(onProjectionChange).toHaveBeenCalledWith(appliedProjection),
  );
  expect(redo).not.toHaveBeenCalled();
});

test("cancels queued Undo when Album Design Apply fails", async () => {
  const pendingApply = deferredProjection();
  const dialog = projectDialogHarness();
  const projectSessionPort = projectSessionPortWithApply(
    () => pendingApply.promise,
  );
  const undo = vi.fn<ProjectSessionPort["undo"]>(async () => projection);
  projectSessionPort.undo = undo;

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.click(albumDesign.getByRole("button", { name: "Aplicar" }));
  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });

  const failure = new Error("O Design do Álbum não pôde ser aplicado.");
  await act(async () => {
    pendingApply.reject(failure);
    await pendingApply.promise.catch(() => undefined);
  });

  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "projectOperationFailure",
      message: failure.message,
    }),
  );
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(undo).not.toHaveBeenCalled();
});

test("waits for a pending Album Design Apply before requesting Project close", async () => {
  const pendingApply = deferredProjection();
  const appliedProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      dirty: true,
      revision: projection.state.revision + 1,
    },
  };
  const close = projectWindowHarness();

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(() => pendingApply.promise)}
      projectDialogPort={close.dialog.port}
      projectWindowPort={close.port}
      onProjectionChange={() => undefined}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.click(albumDesign.getByRole("button", { name: "Aplicar" }));
  fireEvent.click(getApplicationCommand("Arquivo", "Fechar Projeto"));

  expect(close.port.requestClose).not.toHaveBeenCalled();

  await act(async () => {
    pendingApply.resolve(appliedProjection);
    await pendingApply.promise;
  });

  await waitFor(() => expect(close.port.requestClose).toHaveBeenCalledOnce());
  expect(close.dialog.present).toHaveBeenCalledWith({
    busy: false,
    kind: "projectCloseConfirmation",
  });
});

test("cancels a queued Project close after Album Design Apply fails and allows retry", async () => {
  const pendingApply = deferredProjection();
  const close = projectWindowHarness();

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(() => pendingApply.promise)}
      projectDialogPort={close.dialog.port}
      projectWindowPort={close.port}
      onProjectionChange={() => undefined}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.click(albumDesign.getByRole("button", { name: "Aplicar" }));
  fireEvent.click(getApplicationCommand("Arquivo", "Fechar Projeto"));

  expect(close.port.requestClose).not.toHaveBeenCalled();

  const failure = new Error("O Design do Álbum não pôde ser aplicado.");
  await act(async () => {
    pendingApply.reject(failure);
    await pendingApply.promise.catch(() => undefined);
  });

  await waitFor(() =>
    expect(close.dialog.present).toHaveBeenCalledWith({
      kind: "projectOperationFailure",
      message: failure.message,
    }),
  );
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(close.port.requestClose).not.toHaveBeenCalled();
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeEnabled(),
  );
  const dismissCount = close.dialog.dismiss.mock.calls.length;
  act(() => close.dialog.emit("dismissProjectOperationFailure"));
  await waitFor(() =>
    expect(close.dialog.dismiss).toHaveBeenCalledTimes(dismissCount + 1),
  );

  fireEvent.click(getApplicationCommand("Arquivo", "Fechar Projeto"));

  await waitFor(() => expect(close.port.requestClose).toHaveBeenCalledOnce());
  expect(close.dialog.present).toHaveBeenLastCalledWith({
    busy: false,
    kind: "projectCloseConfirmation",
  });
});

test("releases a native close request when pending Album Design Apply fails", async () => {
  const pendingApply = deferredProjection();
  const close = projectWindowHarness();
  const onProjectionChange = vi.fn();
  close.port.resolveClose = vi.fn(async () => ({
    kind: "cancelled" as const,
    projection,
  }));

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(() => pendingApply.promise)}
      projectDialogPort={close.dialog.port}
      projectWindowPort={close.port}
      onProjectionChange={onProjectionChange}
    />,
  );

  await waitFor(() =>
    expect(close.port.onCloseRequested).toHaveBeenCalledOnce(),
  );
  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.click(albumDesign.getByRole("button", { name: "Aplicar" }));
  close.emitCloseRequested();

  expect(close.port.resolveClose).not.toHaveBeenCalled();

  const failure = new Error("O Design do Álbum não pôde ser aplicado.");
  await act(async () => {
    pendingApply.reject(failure);
    await pendingApply.promise.catch(() => undefined);
  });

  await waitFor(() =>
    expect(close.port.resolveClose).toHaveBeenCalledWith("cancel"),
  );
  expect(onProjectionChange).toHaveBeenCalledWith(projection);
  expect(close.dialog.present).toHaveBeenCalledWith({
    kind: "projectOperationFailure",
    message: failure.message,
  });
  const dismissCount = close.dialog.dismiss.mock.calls.length;
  act(() => close.dialog.emit("dismissProjectOperationFailure"));
  await waitFor(() =>
    expect(close.dialog.dismiss).toHaveBeenCalledTimes(dismissCount + 1),
  );

  close.emitCloseRequested();

  await waitFor(() =>
    expect(close.dialog.present).toHaveBeenCalledWith({
      busy: false,
      kind: "projectCloseConfirmation",
    }),
  );
});

test("maps Borda zero to none and a positive value back to solid", async () => {
  const projectionWithBorder: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          frameBorder: { kind: "solid", rgb: "#C5A46D", widthUm: 2_500 },
        },
      },
    },
    composition: {
      ...projection.composition,
      frameBorder: { kind: "solid", rgb: "#C5A46D", widthUm: 2_500 },
    },
  };
  const projectionWithoutBorder: EditorProjection = {
    ...projectionWithBorder,
    state: {
      ...projectionWithBorder.state,
      album: {
        ...projectionWithBorder.state.album,
        visualDefaults: {
          ...projectionWithBorder.state.album.visualDefaults,
          frameBorder: { kind: "none" },
        },
      },
    },
    composition: {
      ...projectionWithBorder.composition,
      frameBorder: { kind: "none" },
    },
  };
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () =>
    projectionWithoutBorder,
  );
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithBorder}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const design = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  const borderWidth = design.getByRole("slider", {
    name: "Espessura da Borda",
  });
  const applyDesign = design.getByRole("button", { name: "Aplicar" });

  fireEvent.change(borderWidth, { target: { value: "0" } });
  expect(design.getByText("sem borda")).toBeVisible();
  fireEvent.click(applyDesign);
  await waitFor(() =>
    expect(apply).toHaveBeenLastCalledWith({
      kind: "setVisualDefaults",
      visualDefaults: {
        ...projectionWithBorder.state.album.visualDefaults,
        frameBorder: { kind: "none" },
      },
    }, expect.any(Function)),
  );

  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithoutBorder}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );
  fireEvent.change(
    design.getByRole("slider", { name: "Espessura da Borda" }),
    { target: { value: "1250" } },
  );
  expect(design.getByText("1.25 mm")).toBeVisible();
  fireEvent.click(applyDesign);
  await waitFor(() =>
    expect(apply).toHaveBeenLastCalledWith({
      kind: "setVisualDefaults",
      visualDefaults: {
        ...projectionWithBorder.state.album.visualDefaults,
        frameBorder: {
          kind: "solid",
          rgb: "#C5A46D",
          widthUm: 1_250,
        },
      },
    }, expect.any(Function)),
  );
});

test("keeps Espaço entre Frames as a preview-only placeholder", () => {
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () => projection);
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const designSection = screen
    .getByRole("button", { name: "Design do Álbum" })
    .closest("section") as HTMLElement;
  const design = within(designSection);
  const gap = design.getByRole("slider", { name: "Espaço entre Frames" });
  const secondFrame = design.getByLabelText("Frame demonstrativo esquerdo 2");
  const initialSecondFrameX = Number(secondFrame.getAttribute("x"));
  const applyDesign = design.getByRole("button", { name: "Aplicar" });

  expect(gap.closest("label")).toHaveAttribute(
    "data-placeholder-feature",
    "album-design-frame-gap",
  );
  fireEvent.change(gap, { target: { value: "18000" } });

  expect(design.getByText("18 mm")).toBeVisible();
  expect(Number(secondFrame.getAttribute("x"))).toBeGreaterThan(
    initialSecondFrameX,
  );
  expect(applyDesign).toBeDisabled();
  expect(apply).not.toHaveBeenCalled();
});

test("coordinates Decorative popups, placeholder import and focus restoration", async () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={decorativeProjection}
      projectSessionPort={projectSessionPortWithApply(async () =>
        decorativeProjection
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const designSection = screen
    .getByRole("button", { name: "Design do Álbum" })
    .closest("section") as HTMLElement;
  const design = within(designSection);
  const backgroundTrigger = design.getByRole("button", {
    name: "Escolher Decorativo para Background",
  });
  const overlayTrigger = design.getByRole("button", {
    name: /Decorativo do Overlay: Overlay translúcido\.png/,
  });

  fireEvent.click(backgroundTrigger);
  const backgroundMenu = design.getByRole("menu", {
    name: "Decorativos para Background",
  });
  const importPlaceholder = within(backgroundMenu).getByRole("menuitem", {
    name: "Importar Decorativo",
  });
  expect(importPlaceholder).toBeDisabled();
  expect(importPlaceholder).toHaveAttribute(
    "data-placeholder-feature",
    "import-decorative-files",
  );

  fireEvent.click(overlayTrigger);
  expect(backgroundMenu).not.toBeInTheDocument();
  const overlayMenu = design.getByRole("menu", {
    name: "Decorativos para Overlay",
  });
  await waitFor(() =>
    expect(within(overlayMenu).getAllByRole("menuitem")[0]).toHaveFocus(),
  );

  fireEvent.keyDown(document, { key: "Escape" });
  expect(overlayMenu).not.toBeInTheDocument();
  expect(overlayTrigger).toHaveFocus();

  fireEvent.click(backgroundTrigger);
  fireEvent.pointerDown(design.getByText("Padrões visuais"));
  expect(
    design.queryByRole("menu", { name: "Decorativos para Background" }),
  ).not.toBeInTheDocument();
  await waitFor(() => expect(backgroundTrigger).toHaveFocus());
});

test("presents an empty per-side Overlay as absent", () => {
  const projectionWithoutOverlay: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          overlay: { scope: "perSide", left: null, right: null },
        },
      },
    },
  };

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithoutOverlay}
      projectSessionPort={projectSessionPortWithApply(
        async () => projectionWithoutOverlay,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const visualDefaults = screen
    .getByRole("button", { name: "Design do Álbum" })
    .closest("section") as HTMLElement;
  expect(
    within(visualDefaults).getByRole("button", { name: "Sem Overlay" }),
  ).toHaveAttribute("aria-pressed", "true");
  expect(
    visualDefaults.querySelector(".visual-default-picker__preview--none"),
  ).toBeInTheDocument();
});

test("does not present divergent per-side Overlays as absent", () => {
  const projectionWithMixedOverlay: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          overlay: {
            scope: "perSide",
            left: { kind: "media", mediaId: "decorative-overlay" },
            right: null,
          },
        },
      },
    },
  };

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithMixedOverlay}
      projectSessionPort={projectSessionPortWithApply(
        async () => projectionWithMixedOverlay,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const visualDefaults = screen
    .getByRole("button", { name: "Design do Álbum" })
    .closest("section") as HTMLElement;
  // Um lado tem Overlay e o outro não: o escopo é misto, não é ausência.
  expect(
    within(visualDefaults).getByRole("button", { name: "Sem Overlay" }),
  ).toHaveAttribute("aria-pressed", "false");
});

test("confirms and applies Album information as one authoritative Project change", async () => {
  const initialProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      revision: 0,
      savedRevision: 0,
      dirty: false,
      canUndo: false,
      canRedo: false,
    },
  };
  const changedProjection: EditorProjection = {
    ...initialProjection,
    state: {
      ...initialProjection.state,
      document: {
        ...initialProjection.state.document,
        dpi: 600,
      },
      revision: 1,
      dirty: true,
      canUndo: true,
    },
  };
  const apply = vi.fn(async () => changedProjection);
  const projectSessionPort = projectSessionPortWithApply(apply);
  const dialog = projectDialogHarness();
  const projectCorePort = projectCorePortWithApply(apply);
  const onProjectionChange = vi.fn();

  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={initialProjection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      projectCorePort={projectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );

  const albumInformationTrigger = screen.getByRole("button", {
    name: "Informações do Álbum",
  });
  if (albumInformationTrigger.getAttribute("aria-expanded") !== "true") {
    fireEvent.click(albumInformationTrigger);
  }
  const albumInformationBeforeApply = within(
    albumInformationTrigger.closest("section") as HTMLElement,
  );
  const input = albumInformationBeforeApply.getByRole("textbox", {
    name: "DPI",
  });
  fireEvent.change(input, { target: { value: "600" } });
  expect(apply).not.toHaveBeenCalled();
  await waitFor(() =>
    expect(
      albumInformationBeforeApply.getByRole("button", { name: "Aplicar" }),
    ).toBeEnabled(),
  );

  await act(async () => {
    fireEvent.click(
      albumInformationBeforeApply.getByRole("button", { name: "Aplicar" }),
    );
    await Promise.resolve();
  });

  expect(apply).not.toHaveBeenCalled();
  expect(dialog.present).toHaveBeenLastCalledWith(
    expect.objectContaining({
      busy: false,
      kind: "albumInformationConfirmation",
    }),
  );

  await act(async () => {
    dialog.emit("confirmAlbumInformation");
    await Promise.resolve();
    await Promise.resolve();
  });

  expect(apply).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith({
    kind: "setAlbumInformation",
    information: {
      displayUnit: "mm",
      sheetWidthUm: initialProjection.state.document.sheetWidthUm,
      sheetHeightUm: initialProjection.state.document.sheetHeightUm,
      dpi: 600,
      bleedUm: initialProjection.state.document.bleedUm,
      safetyUm: initialProjection.state.document.safetyUm,
      firstSheet: "double",
      lastSheet: "double",
    },
  }, expect.any(Function));
  expect(onProjectionChange).toHaveBeenCalledWith(changedProjection);

  view.rerender(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={changedProjection}
      projectCorePort={projectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );

  const albumInformation = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  expect(albumInformation.getByRole("textbox", { name: "DPI" })).toHaveValue(
    "600",
  );
  expect(getApplicationCommand("Editar", "Desfazer")).toBeEnabled();
});

test("saves the visible revision and applies the authoritative saved projection", async () => {
  const savedProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      savedRevision: projection.state.revision,
      dirty: false,
      canUndo: true,
    },
  };
  const save = vi.fn<ProjectCorePort["save"]>(async () => ({
    outcome: {
      kind: "saved",
      revision: savedProjection.state.revision,
    },
    projection: savedProjection,
  }));
  const projectCorePort = projectCorePortWithApply(
    async () => projection,
  );
  projectCorePort.save = save;
  const onProjectionChange = vi.fn();

  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );

  const saveCommand = getApplicationCommand("Arquivo", "Salvar");
  await act(async () => {
    fireEvent.click(saveCommand);
    await Promise.resolve();
  });

  expect(save).toHaveBeenCalledWith(projection.state.revision);
  expect(onProjectionChange).toHaveBeenCalledWith(savedProjection);

  view.rerender(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={savedProjection}
      projectCorePort={projectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );
  expect(getApplicationCommand("Editar", "Desfazer")).toBeEnabled();
});

test("saves with Ctrl+S without transient feedback or flashing unrelated controls", async () => {
  const savedProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      savedRevision: projection.state.revision,
      dirty: false,
    },
  };
  let finishSave!: (
    result: Awaited<ReturnType<ProjectSessionPort["save"]>>,
  ) => void;
  const pendingSave = new Promise<
    Awaited<ReturnType<ProjectSessionPort["save"]>>
  >((resolve) => {
    finishSave = resolve;
  });
  const save = vi.fn<ProjectSessionPort["save"]>(() => pendingSave);
  const projectSessionPort = projectSessionPortWithApply(async () => projection);
  projectSessionPort.save = save;
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );

  const exportButton = screen.getByRole("button", {
    name: "Exportar Lâmina",
  });
  await act(async () => {
    fireEvent.keyDown(window, { ctrlKey: true, key: "s" });
    await Promise.resolve();
  });

  expect(save).toHaveBeenCalledWith(projection.state.revision);
  expect(screen.queryByText("Salvando")).not.toBeInTheDocument();
  expect(screen.queryByText("Aguarde…")).not.toBeInTheDocument();
  expect(exportButton).toBeEnabled();

  await act(async () => {
    finishSave({
      outcome: {
        kind: "saved",
        revision: savedProjection.state.revision,
      },
      projection: savedProjection,
    });
    await pendingSave;
  });
  expect(onProjectionChange).toHaveBeenCalledWith(savedProjection);
});

test("keeps unrelated controls stable while a History command is pending", async () => {
  const pendingUndo = deferredProjection();
  const projectSessionPort = projectSessionPortWithApply(async () => projection);
  const undo = vi.fn<ProjectSessionPort["undo"]>(() => pendingUndo.promise);
  projectSessionPort.undo = undo;
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );

  const exportButton = screen.getByRole("button", {
    name: "Exportar Lâmina",
  });
  await act(async () => {
    fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
    await Promise.resolve();
  });

  expect(undo).toHaveBeenCalledOnce();
  expect(screen.queryByText("Desfazendo")).not.toBeInTheDocument();
  expect(exportButton).toBeEnabled();

  await act(async () => {
    pendingUndo.resolve(projection);
    await pendingUndo.promise;
  });
  expect(onProjectionChange).toHaveBeenCalledWith(projection);
});

test("preserves both unapplied Album drafts when Save returns an equivalent projection", async () => {
  const savedProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      document: { ...projection.state.document },
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          background: { ...projection.state.album.visualDefaults.background },
          overlay: { ...projection.state.album.visualDefaults.overlay },
          frameBorder: { ...projection.state.album.visualDefaults.frameBorder },
        },
      },
      savedRevision: projection.state.revision,
      dirty: false,
    },
  };
  const save = vi.fn<ProjectSessionPort["save"]>(async () => ({
    outcome: { kind: "saved", revision: savedProjection.state.revision },
    projection: savedProjection,
  }));
  const projectSessionPort = projectSessionPortWithApply(async () => projection);
  projectSessionPort.save = save;
  const onProjectionChange = vi.fn();
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );

  const information = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  const design = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  fireEvent.change(design.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  expect(design.getByRole("button", { name: "Aplicar" })).toBeEnabled();

  fireEvent.keyDown(window, { ctrlKey: true, key: "s" });
  await waitFor(() => expect(onProjectionChange).toHaveBeenCalledWith(savedProjection));
  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={savedProjection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );

  expect(information.getByLabelText("DPI")).toHaveValue("600");
  expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled();
  expect(design.getByLabelText("Cor do Background")).toHaveValue("#f7f5f0");
  expect(design.getByRole("button", { name: "Aplicar" })).toBeEnabled();
});

test("preserves the Album Information draft when Album Design is applied", async () => {
  const appliedDesignProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      revision: projection.state.revision + 1,
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          background: {
            scope: "bothSides",
            both: { kind: "color", rgb: "#F7F5F0" },
          },
        },
      },
    },
  };
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () =>
    appliedDesignProjection,
  );
  const projectSessionPort = projectSessionPortWithApply(apply);
  const onProjectionChange = vi.fn();
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );
  const information = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  const design = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  fireEvent.change(design.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );

  fireEvent.click(design.getByRole("button", { name: "Aplicar" }));
  await waitFor(() =>
    expect(onProjectionChange).toHaveBeenCalledWith(appliedDesignProjection),
  );
  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={appliedDesignProjection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );

  expect(information.getByLabelText("DPI")).toHaveValue("600");
  await waitFor(() =>
    expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
});

test("preserves the Album Design draft when Album Information is applied", async () => {
  const appliedInformationProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      revision: projection.state.revision + 1,
      document: { ...projection.state.document, dpi: 600 },
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          background: { ...projection.state.album.visualDefaults.background },
          overlay: { ...projection.state.album.visualDefaults.overlay },
          frameBorder: { ...projection.state.album.visualDefaults.frameBorder },
        },
      },
    },
  };
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () =>
    appliedInformationProjection,
  );
  const projectSessionPort = projectSessionPortWithApply(apply);
  const dialog = projectDialogHarness();
  const onProjectionChange = vi.fn();
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );
  const information = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  const design = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(design.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  await waitFor(() =>
    expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(information.getByRole("button", { name: "Aplicar" }));
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "albumInformationConfirmation" }),
    ),
  );
  dialog.emit("confirmAlbumInformation");
  await waitFor(() =>
    expect(onProjectionChange).toHaveBeenCalledWith(appliedInformationProjection),
  );
  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={appliedInformationProjection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );

  expect(design.getByLabelText("Cor do Background")).toHaveValue("#f7f5f0");
  expect(design.getByRole("button", { name: "Aplicar" })).toBeEnabled();
});

test("preserves both unapplied Album drafts across equivalent Undo and Redo projections", async () => {
  function equivalentHistoryProjection(
    revision: number,
    canUndo: boolean,
    canRedo: boolean,
  ): EditorProjection {
    return {
      ...projection,
      state: {
        ...projection.state,
        canRedo,
        canUndo,
        revision,
        document: { ...projection.state.document },
        album: {
          ...projection.state.album,
          visualDefaults: {
            ...projection.state.album.visualDefaults,
            background: { ...projection.state.album.visualDefaults.background },
            overlay: { ...projection.state.album.visualDefaults.overlay },
            frameBorder: { ...projection.state.album.visualDefaults.frameBorder },
          },
        },
      },
    };
  }

  const afterUndo = equivalentHistoryProjection(24, false, true);
  const afterRedo = equivalentHistoryProjection(25, true, false);
  const projectSessionPort = projectSessionPortWithApply(async () => projection);
  projectSessionPort.undo = vi.fn(async () => afterUndo);
  projectSessionPort.redo = vi.fn(async () => afterRedo);
  const onProjectionChange = vi.fn();
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );
  const information = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  const design = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  fireEvent.change(design.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );

  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  await waitFor(() => expect(onProjectionChange).toHaveBeenCalledWith(afterUndo));
  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={afterUndo}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );
  expect(information.getByLabelText("DPI")).toHaveValue("600");
  expect(design.getByLabelText("Cor do Background")).toHaveValue("#f7f5f0");

  fireEvent.keyDown(window, { ctrlKey: true, key: "y" });
  await waitFor(() => expect(onProjectionChange).toHaveBeenCalledWith(afterRedo));
  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={afterRedo}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );
  expect(information.getByLabelText("DPI")).toHaveValue("600");
  expect(design.getByLabelText("Cor do Background")).toHaveValue("#f7f5f0");
});

test("keeps a mutation failure behind Album Information and releases both owners in order", async () => {
  const pendingDesign = deferredProjection();
  const dialog = projectDialogHarness();
  const projectSessionPort = projectSessionPortWithApply(
    () => pendingDesign.promise,
  );

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );

  const design = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(design.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.click(design.getByRole("button", { name: "Aplicar" }));

  const information = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  await waitFor(() =>
    expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(information.getByRole("button", { name: "Aplicar" }));
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "albumInformationConfirmation" }),
    ),
  );

  const failure = new Error("O Design do Álbum não pôde ser aplicado.");
  await act(async () => {
    pendingDesign.reject(failure);
    await pendingDesign.promise.catch(() => undefined);
  });
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "projectOperationFailure",
      message: failure.message,
    }),
  );

  const dismissalsBeforeStaleAction = dialog.dismiss.mock.calls.length;
  act(() => dialog.emit("dismissProjectOperationFailure"));
  expect(dialog.dismiss).toHaveBeenCalledTimes(dismissalsBeforeStaleAction);
  expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeDisabled();

  act(() => dialog.emit("cancelAlbumInformation"));
  await waitFor(() =>
    expect(dialog.dismiss).toHaveBeenCalledTimes(
      dismissalsBeforeStaleAction + 1,
    ),
  );
  act(() => dialog.emit("dismissProjectOperationFailure"));
  await waitFor(() =>
    expect(dialog.dismiss).toHaveBeenCalledTimes(
      dismissalsBeforeStaleAction + 2,
    ),
  );
  expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeEnabled();
});

test("queues native Close behind Export and routes every action to its owning session", async () => {
  let emitExport!: (event: ExportProgressEvent) => void;
  let finishExport!: (outcome: ExportOutcome) => void;
  const completion = new Promise<ExportOutcome>((resolve) => {
    finishExport = resolve;
  });
  const cancel = vi.fn(async () => "requested" as const);
  const close = projectWindowHarness();
  close.port.resolveClose = vi.fn(async () => ({
    kind: "cancelled" as const,
    projection,
  }));
  const controlledExportPort: LegacyExportPort = {
    startSheet: (_sheetId, onEvent) => {
      emitExport = onEvent;
      return { cancel, completion };
    },
  };

  render(
    <ProjectWorkspace
      exportPort={controlledExportPort}
      projection={projection}
      projectDialogPort={close.dialog.port}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectWindowPort={close.port}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "Exportar Lâmina" }));
  act(() => emitExport({ event: "started", cancellable: true }));
  await waitFor(() =>
    expect(close.dialog.present).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "exportProgress" }),
    ),
  );

  act(() => close.emitCloseRequested());
  await waitFor(() =>
    expect(close.dialog.present).toHaveBeenCalledWith({
      busy: false,
      kind: "projectCloseConfirmation",
    }),
  );

  act(() => close.dialog.emit("cancelProjectClose"));
  expect(close.port.resolveClose).not.toHaveBeenCalled();

  act(() => close.dialog.emit("cancelExport"));
  expect(cancel).toHaveBeenCalledOnce();
  await act(async () => {
    finishExport({ status: "cancelled" });
    await completion;
  });
  await act(async () => {
    close.dialog.emit("dismissExport");
    await Promise.resolve();
  });

  act(() => close.dialog.emit("cancelProjectClose"));
  await waitFor(() =>
    expect(close.port.resolveClose).toHaveBeenCalledWith("cancel"),
  );
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Editar" })).toBeEnabled(),
  );
});

test("materializes an Album Design draft over the projection produced by a pending Undo", async () => {
  const pendingUndo = deferredProjection();
  const afterUndo: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      canRedo: true,
      canUndo: false,
      revision: projection.state.revision - 1,
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          background: {
            scope: "perSide",
            left: { kind: "color", rgb: "#AABBCC" },
            right: { kind: "color", rgb: "#223344" },
          },
          overlay: {
            scope: "bothSides",
            both: { kind: "media", mediaId: "history-overlay" },
          },
          frameBorder: {
            kind: "solid",
            rgb: "#445566",
            widthUm: 2_000,
          },
        },
      },
    },
  };
  const appliedProjection: EditorProjection = {
    ...afterUndo,
    state: {
      ...afterUndo.state,
      revision: afterUndo.state.revision + 1,
      album: {
        ...afterUndo.state.album,
        visualDefaults: {
          ...afterUndo.state.album.visualDefaults,
          background: {
            scope: "perSide",
            left: { kind: "color", rgb: "#F7F5F0" },
            right: { kind: "color", rgb: "#223344" },
          },
        },
      },
    },
  };
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () =>
    appliedProjection,
  );
  const projectSessionPort = projectSessionPortWithApply(apply);
  projectSessionPort.undo = vi.fn(() => pendingUndo.promise);

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );
  const design = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.click(design.getByRole("button", { name: "Lado esquerdo" }));
  fireEvent.change(design.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  await waitFor(() => expect(projectSessionPort.undo).toHaveBeenCalledOnce());
  fireEvent.click(design.getByRole("button", { name: "Aplicar" }));
  expect(apply).not.toHaveBeenCalled();

  await act(async () => {
    pendingUndo.resolve(afterUndo);
    await pendingUndo.promise;
  });

  await waitFor(() =>
    expect(apply).toHaveBeenCalledWith({
      kind: "setVisualDefaults",
      visualDefaults: {
        background: {
          scope: "perSide",
          left: { kind: "color", rgb: "#F7F5F0" },
          right: { kind: "color", rgb: "#223344" },
        },
        overlay: afterUndo.state.album.visualDefaults.overlay,
        frameBorder: afterUndo.state.album.visualDefaults.frameBorder,
      },
    }, expect.any(Function)),
  );
});

test("applies an Album Design draft over its captured baseline when pending Undo fails", async () => {
  const pendingUndo = deferredProjection();
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () => projection);
  const projectSessionPort = projectSessionPortWithApply(apply);
  projectSessionPort.undo = vi.fn(() => pendingUndo.promise);

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );
  const design = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(design.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  await waitFor(() => expect(projectSessionPort.undo).toHaveBeenCalledOnce());
  fireEvent.click(design.getByRole("button", { name: "Aplicar" }));

  await act(async () => {
    pendingUndo.reject(new Error("Undo indisponível."));
    await pendingUndo.promise.catch(() => undefined);
  });

  await waitFor(() =>
    expect(apply).toHaveBeenCalledWith({
      kind: "setVisualDefaults",
      visualDefaults: {
        background: {
          scope: "bothSides",
          both: { kind: "color", rgb: "#F7F5F0" },
        },
        overlay: projection.state.album.visualDefaults.overlay,
        frameBorder: projection.state.album.visualDefaults.frameBorder,
      },
    }, expect.any(Function)),
  );
});

test("materializes an Album Information draft over the projection produced by a pending Redo", async () => {
  const beforeRedo: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      canRedo: true,
      canUndo: false,
      revision: projection.state.revision - 1,
    },
  };
  const afterRedo: EditorProjection = {
    ...beforeRedo,
    state: {
      ...beforeRedo.state,
      canRedo: false,
      canUndo: true,
      revision: beforeRedo.state.revision + 1,
      document: {
        ...beforeRedo.state.document,
        bleedUm: 5_000,
        safetyUm: 7_000,
      },
    },
  };
  const appliedProjection: EditorProjection = {
    ...afterRedo,
    state: {
      ...afterRedo.state,
      revision: afterRedo.state.revision + 1,
      document: { ...afterRedo.state.document, dpi: 600 },
    },
  };
  const pendingRedo = deferredProjection();
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () =>
    appliedProjection,
  );
  const projectSessionPort = projectSessionPortWithApply(apply);
  projectSessionPort.redo = vi.fn(() => pendingRedo.promise);
  const dialog = projectDialogHarness();

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={beforeRedo}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );
  const information = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  await waitFor(() =>
    expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.keyDown(window, { ctrlKey: true, key: "y" });
  await waitFor(() => expect(projectSessionPort.redo).toHaveBeenCalledOnce());
  fireEvent.click(information.getByRole("button", { name: "Aplicar" }));
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "albumInformationConfirmation" }),
    ),
  );
  dialog.emit("confirmAlbumInformation");
  expect(apply).not.toHaveBeenCalled();

  await act(async () => {
    pendingRedo.resolve(afterRedo);
    await pendingRedo.promise;
  });

  await waitFor(() =>
    expect(apply).toHaveBeenCalledWith({
      kind: "setAlbumInformation",
      information: {
        displayUnit: afterRedo.state.document.displayUnit,
        sheetWidthUm: afterRedo.state.document.sheetWidthUm,
        sheetHeightUm: afterRedo.state.document.sheetHeightUm,
        dpi: 600,
        bleedUm: 5_000,
        safetyUm: 7_000,
        firstSheet: "double",
        lastSheet: "double",
      },
    }, expect.any(Function)),
  );
});

test("applies an Album Information draft over its captured baseline when pending Redo fails", async () => {
  const beforeRedo: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      canRedo: true,
      canUndo: false,
      revision: projection.state.revision - 1,
    },
  };
  const pendingRedo = deferredProjection();
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () => projection);
  const projectSessionPort = projectSessionPortWithApply(apply);
  projectSessionPort.redo = vi.fn(() => pendingRedo.promise);
  const dialog = projectDialogHarness();

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={beforeRedo}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );
  const information = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  await waitFor(() =>
    expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.keyDown(window, { ctrlKey: true, key: "y" });
  await waitFor(() => expect(projectSessionPort.redo).toHaveBeenCalledOnce());
  fireEvent.click(information.getByRole("button", { name: "Aplicar" }));
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "albumInformationConfirmation" }),
    ),
  );
  dialog.emit("confirmAlbumInformation");

  await act(async () => {
    pendingRedo.reject(new Error("Redo indisponível."));
    await pendingRedo.promise.catch(() => undefined);
  });

  await waitFor(() =>
    expect(apply).toHaveBeenCalledWith({
      kind: "setAlbumInformation",
      information: {
        displayUnit: beforeRedo.state.document.displayUnit,
        sheetWidthUm: beforeRedo.state.document.sheetWidthUm,
        sheetHeightUm: beforeRedo.state.document.sheetHeightUm,
        dpi: 600,
        bleedUm: beforeRedo.state.document.bleedUm,
        safetyUm: beforeRedo.state.document.safetyUm,
        firstSheet: "double",
        lastSheet: "double",
      },
    }, expect.any(Function)),
  );
});

test("revalidates materialized Album Information after pending History and blocks an invalid Apply", async () => {
  const beforeRedo: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      canRedo: true,
      canUndo: false,
      revision: projection.state.revision - 1,
    },
  };
  const afterRedo: EditorProjection = {
    ...beforeRedo,
    state: {
      ...beforeRedo.state,
      canRedo: false,
      canUndo: true,
      revision: beforeRedo.state.revision + 1,
      document: {
        ...beforeRedo.state.document,
        sheetWidthUm: 2_000_000,
      },
    },
  };
  const pendingRedo = deferredProjection();
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () => afterRedo);
  const validateAlbumInformation = vi.fn<
    ProjectSessionPort["validateAlbumInformation"]
  >(async (information) =>
    information.sheetWidthUm === afterRedo.state.document.sheetWidthUm &&
    information.dpi === 600
      ? { errors: ["sheetWidthRasterOutOfRange"], impact: null }
      : {
          errors: [],
          impact: {
            sheetWidthPx: 7_087,
            pageWidthPx: 3_543,
            heightPx: 3_543,
          },
        },
  );
  const projectSessionPort = projectSessionPortWithApply(apply);
  projectSessionPort.redo = vi.fn(() => pendingRedo.promise);
  projectSessionPort.validateAlbumInformation = validateAlbumInformation;
  const dialog = projectDialogHarness();

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={beforeRedo}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );
  const information = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  await waitFor(() =>
    expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.keyDown(window, { ctrlKey: true, key: "y" });
  await waitFor(() => expect(projectSessionPort.redo).toHaveBeenCalledOnce());
  fireEvent.click(information.getByRole("button", { name: "Aplicar" }));
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "albumInformationConfirmation" }),
    ),
  );
  dialog.emit("confirmAlbumInformation");

  await act(async () => {
    pendingRedo.resolve(afterRedo);
    await pendingRedo.promise;
  });

  await waitFor(() =>
    expect(validateAlbumInformation).toHaveBeenCalledWith({
      displayUnit: afterRedo.state.document.displayUnit,
      sheetWidthUm: 2_000_000,
      sheetHeightUm: afterRedo.state.document.sheetHeightUm,
      dpi: 600,
      bleedUm: afterRedo.state.document.bleedUm,
      safetyUm: afterRedo.state.document.safetyUm,
      firstSheet: "double",
      lastSheet: "double",
    }),
  );
  expect(apply).not.toHaveBeenCalled();
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "projectOperationFailure",
      message:
        "As Informações do Álbum mudaram enquanto a confirmação estava aberta e precisam ser revistas antes de Aplicar.",
    }),
  );
});

test("updates a stale Album Information summary and requires reconfirmation after History", async () => {
  const beforeRedo: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      canRedo: true,
      canUndo: false,
      revision: projection.state.revision - 1,
    },
  };
  const afterRedo: EditorProjection = {
    ...beforeRedo,
    state: {
      ...beforeRedo.state,
      canRedo: false,
      canUndo: true,
      revision: beforeRedo.state.revision + 1,
      document: { ...beforeRedo.state.document, dpi: 400 },
    },
  };
  const appliedProjection: EditorProjection = {
    ...afterRedo,
    state: {
      ...afterRedo.state,
      revision: afterRedo.state.revision + 1,
      document: { ...afterRedo.state.document, dpi: 600 },
    },
  };
  const pendingRedo = deferredProjection();
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () =>
    appliedProjection,
  );
  const projectSessionPort = projectSessionPortWithApply(apply);
  projectSessionPort.redo = vi.fn(() => pendingRedo.promise);
  const dialog = projectDialogHarness();

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={beforeRedo}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );
  const information = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  await waitFor(() =>
    expect(information.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.keyDown(window, { ctrlKey: true, key: "y" });
  await waitFor(() => expect(projectSessionPort.redo).toHaveBeenCalledOnce());
  fireEvent.click(information.getByRole("button", { name: "Aplicar" }));
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "albumInformationConfirmation" }),
    ),
  );
  dialog.emit("confirmAlbumInformation");

  await act(async () => {
    pendingRedo.resolve(afterRedo);
    await pendingRedo.promise;
  });

  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      busy: false,
      details: expect.arrayContaining([
        { label: "DPI", value: "400 → 600" },
      ]),
      kind: "albumInformationConfirmation",
    }),
  );
  expect(apply).not.toHaveBeenCalled();

  dialog.emit("confirmAlbumInformation");

  await waitFor(() =>
    expect(apply).toHaveBeenCalledWith({
      kind: "setAlbumInformation",
      information: {
        displayUnit: afterRedo.state.document.displayUnit,
        sheetWidthUm: afterRedo.state.document.sheetWidthUm,
        sheetHeightUm: afterRedo.state.document.sheetHeightUm,
        dpi: 600,
        bleedUm: afterRedo.state.document.bleedUm,
        safetyUm: afterRedo.state.document.safetyUm,
        firstSheet: "double",
        lastSheet: "double",
      },
    }, expect.any(Function)),
  );
});

test("uses the native Salvar como flow and adopts the new Project projection", async () => {
  const savedAsProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      projectId: "81f68858-c8f5-4fcb-8e0f-185c3ff45cf5",
      projectName: "Versão independente",
      savedRevision: projection.state.revision,
      dirty: false,
      canUndo: true,
    },
  };
  const saveAs = vi.fn<ProjectCorePort["saveAs"]>(async () => ({
    outcome: {
      kind: "savedAs",
      previousProjectId: projection.state.projectId,
      projectId: savedAsProjection.state.projectId,
      revision: savedAsProjection.state.revision,
    },
    projection: savedAsProjection,
  }));
  const projectCorePort = projectCorePortWithApply(async () => projection);
  projectCorePort.saveAs = saveAs;
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );

  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivo" }));
  await act(async () => {
    fireEvent.click(screen.getByRole("menuitem", { name: "Salvar como…" }));
    await Promise.resolve();
  });

  expect(saveAs).toHaveBeenCalledWith(projection.state.revision);
  expect(onProjectionChange).toHaveBeenCalledWith(savedAsProjection);
});

test("makes Salvar como a terminal barrier after an accepted deferred import", async () => {
  const importedProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      revision: projection.state.revision + 1,
      canUndo: true,
    },
  };
  const savedAsProjection: EditorProjection = {
    ...importedProjection,
    state: {
      ...importedProjection.state,
      projectId: "81f68858-c8f5-4fcb-8e0f-185c3ff45cf5",
      projectName: "Versão independente",
      savedRevision: importedProjection.state.revision,
      dirty: false,
    },
  };
  type ImportMediaResult = Awaited<
    ReturnType<ProjectCorePort["importMedia"]>
  >;
  let resolveImport!: (result: ImportMediaResult) => void;
  const pendingImport = new Promise<ImportMediaResult>((resolve) => {
    resolveImport = resolve;
  });
  const importMedia = vi.fn<ProjectCorePort["importMedia"]>(
    () => pendingImport,
  );
  const saveAs = vi.fn<ProjectCorePort["saveAs"]>(async () => ({
    outcome: {
      kind: "savedAs",
      previousProjectId: projection.state.projectId,
      projectId: savedAsProjection.state.projectId,
      revision: savedAsProjection.state.revision,
    },
    projection: savedAsProjection,
  }));
  const apply = vi.fn<ProjectCorePort["apply"]>(async () => projection);
  const undo = vi.fn<ProjectCorePort["undo"]>(async () => projection);
  const projectCorePort = projectCorePortWithApply(apply);
  projectCorePort.importMedia = importMedia;
  projectCorePort.saveAs = saveAs;
  projectCorePort.undo = undo;
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );

  act(() => canvasHarness.props?.onEditSheet?.("sheet-001"));
  expect(canvasHarness.props?.mode).toEqual({
    kind: "sheet-editing",
    sheetId: "sheet-001",
  });

  fireEvent.click(screen.getByRole("button", { name: "Importar" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivos…" }));
  await waitFor(() => expect(importMedia).toHaveBeenCalledOnce());

  fireEvent.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  expect(
    screen.getByRole("group", { name: "Filtro, ordem e tamanho" }),
  ).toBeInTheDocument();

  fireEvent.keyDown(window, { ctrlKey: true, shiftKey: true, key: "s" });
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeDisabled(),
  );
  expect(screen.getByRole("button", { name: "Processando…" })).toBeDisabled();

  fireEvent.keyDown(document, { key: "Escape" });
  expect(
    screen.getByRole("group", { name: "Filtro, ordem e tamanho" }),
  ).toBeInTheDocument();
  expect(canvasHarness.props?.mode).toEqual({
    kind: "sheet-editing",
    sheetId: "sheet-001",
  });

  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  fireEvent.doubleClick(screen.getByRole("button", { name: "Campo.jpg" }));
  expect(undo).not.toHaveBeenCalled();
  expect(apply).not.toHaveBeenCalled();

  await act(async () => {
    resolveImport({
      kind: "completed",
      projection: importedProjection,
      mediaIds: ["media-imported"], importedCount: 1, problems: [],
    });
    await pendingImport;
  });

  await waitFor(() =>
    expect(saveAs).toHaveBeenCalledWith(importedProjection.state.revision),
  );
  await act(async () => {
    await Promise.resolve();
  });

  expect(onProjectionChange).toHaveBeenNthCalledWith(1, importedProjection);
  expect(onProjectionChange).toHaveBeenNthCalledWith(2, savedAsProjection);
  expect(undo).not.toHaveBeenCalled();
  expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeDisabled();
});

test("releases the Salvar como barrier after native cancellation", async () => {
  type SaveAsResult = Awaited<ReturnType<ProjectCorePort["saveAs"]>>;
  let resolveSaveAs!: (result: SaveAsResult) => void;
  const pendingSaveAs = new Promise<SaveAsResult>((resolve) => {
    resolveSaveAs = resolve;
  });
  const saveAs = vi.fn<ProjectCorePort["saveAs"]>(() => pendingSaveAs);
  const undo = vi.fn<ProjectCorePort["undo"]>(async () => projection);
  const projectCorePort = projectCorePortWithApply(async () => projection);
  projectCorePort.saveAs = saveAs;
  projectCorePort.undo = undo;

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.keyDown(window, { ctrlKey: true, shiftKey: true, key: "s" });
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeDisabled(),
  );

  await act(async () => {
    resolveSaveAs({ outcome: { kind: "cancelled" }, projection });
    await pendingSaveAs;
  });

  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeEnabled(),
  );
  expect(screen.getByRole("button", { name: "Importar" })).toBeEnabled();

  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  await waitFor(() => expect(undo).toHaveBeenCalledOnce());
});

test("releases the Salvar como barrier after a reported failure", async () => {
  type SaveAsResult = Awaited<ReturnType<ProjectCorePort["saveAs"]>>;
  let rejectSaveAs!: (reason: unknown) => void;
  const pendingSaveAs = new Promise<SaveAsResult>((_resolve, reject) => {
    rejectSaveAs = reject;
  });
  const saveAs = vi.fn<ProjectCorePort["saveAs"]>(() => pendingSaveAs);
  const undo = vi.fn<ProjectCorePort["undo"]>(async () => projection);
  const dialog = projectDialogHarness();
  const projectCorePort = projectCorePortWithApply(async () => projection);
  projectCorePort.saveAs = saveAs;
  projectCorePort.undo = undo;

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
      projectDialogPort={dialog.port}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.keyDown(window, { ctrlKey: true, shiftKey: true, key: "s" });
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeDisabled(),
  );

  await act(async () => {
    rejectSaveAs(
      new SaveProjectError(
        "destination_conflict",
        "Já existe um Projeto no destino escolhido.",
      ),
    );
    await pendingSaveAs.catch(() => undefined);
  });

  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "projectOperationFailure",
      message: "Já existe um Projeto no destino escolhido.",
    }),
  );
  expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "Importar" })).toBeEnabled();

  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  await waitFor(() => expect(undo).toHaveBeenCalledOnce());
});

test("rejects a native close command while the Salvar como barrier is active", async () => {
  type SaveAsResult = Awaited<ReturnType<ProjectCorePort["saveAs"]>>;
  let resolveSaveAs!: (result: SaveAsResult) => void;
  const pendingSaveAs = new Promise<SaveAsResult>((resolve) => {
    resolveSaveAs = resolve;
  });
  const projectCorePort = projectCorePortWithApply(async () => projection);
  projectCorePort.saveAs = vi.fn(() => pendingSaveAs);
  const close = projectWindowHarness();
  close.port.resolveClose = vi.fn(async () => ({
    kind: "cancelled" as const,
    projection,
  }));

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
      projectDialogPort={close.dialog.port}
      projectWindowPort={close.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() =>
    expect(close.port.onCloseRequested).toHaveBeenCalledOnce(),
  );

  fireEvent.keyDown(window, { ctrlKey: true, shiftKey: true, key: "s" });
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeDisabled(),
  );
  act(() => close.emitCloseRequested());

  await waitFor(() =>
    expect(close.port.resolveClose).toHaveBeenCalledWith("cancel"),
  );
  expect(close.dialog.present).not.toHaveBeenCalledWith({
    busy: false,
    kind: "projectCloseConfirmation",
  });

  await act(async () => {
    resolveSaveAs({ outcome: { kind: "cancelled" }, projection });
    await pendingSaveAs;
  });
});

test("uses Ctrl+S for Project save and prevents the browser default", async () => {
  const save = vi.fn<ProjectCorePort["save"]>(async () => ({
    outcome: { kind: "saved", revision: projection.state.revision },
    projection: {
      ...projection,
      state: {
        ...projection.state,
        savedRevision: projection.state.revision,
        dirty: false,
      },
    },
  }));
  const projectCorePort = projectCorePortWithApply(
    async () => projection,
  );
  projectCorePort.save = save;
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
      onProjectionChange={() => undefined}
    />,
  );
  const shortcut = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    ctrlKey: true,
    key: "s",
  });

  await act(async () => {
    window.dispatchEvent(shortcut);
    await Promise.resolve();
  });

  expect(shortcut.defaultPrevented).toBe(true);
  expect(save).toHaveBeenCalledOnce();
  expect(save).toHaveBeenCalledWith(projection.state.revision);
});

test("shows the localized Project save failure", async () => {
  const dialog = projectDialogHarness();
  const projectCorePort = projectCorePortWithApply(
    async () => projection,
  );
  projectCorePort.save = vi.fn(async () => {
    throw new SaveProjectError(
      "persisted_baseline_conflict",
      "O arquivo do Projeto foi alterado fora do MyAlbuns. O Salvamento não substituiu essas alterações.",
    );
  });
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectDialogPort={dialog.port}
      projectCorePort={projectCorePort}
      onProjectionChange={() => undefined}
    />,
  );

  const saveCommand = getApplicationCommand("Arquivo", "Salvar");
  await act(async () => {
    fireEvent.click(saveCommand);
    await Promise.resolve();
  });

  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "projectOperationFailure",
      message:
        "O arquivo do Projeto foi alterado fora do MyAlbuns. O Salvamento não substituiu essas alterações.",
    }),
  );
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("renders each Grade item from its own composed sheet", () => {
  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={twoSheetProjection}
      projectCorePort={projectCorePortWithApply(async () => twoSheetProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(
    screen.getByRole("img", { name: "Prévia da Lâmina 01" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("img", { name: "Prévia da Lâmina 02" }),
  ).toBeInTheDocument();
  const sheetGrid = view.container.querySelector(".sheet-grid") as HTMLElement;
  expect(within(sheetGrid).getAllByRole("img")).toHaveLength(2);
});

test("sizes every Grade tile from the open Sheet proportions", () => {
  const fourByThreeProjection: EditorProjection = {
    ...twoSheetProjection,
    state: {
      ...twoSheetProjection.state,
      document: {
        ...twoSheetProjection.state.document,
        sheetWidthUm: 400_000,
        sheetHeightUm: 300_000,
      },
    },
  };
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={fourByThreeProjection}
      projectSessionPort={projectSessionPortWithApply(
        async () => fourByThreeProjection,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const tiles = Array.from(
    view.container.querySelectorAll<HTMLElement>(".sheet-tile"),
  );
  expect(tiles).not.toHaveLength(0);
  for (const tile of tiles) {
    expect(tile).toHaveStyle({ aspectRatio: "400000 / 300000" });
  }
});

test("presents the Grade with reference metadata and navigation state", () => {
  const projectionWithProjectedPageNumbers: EditorProjection = {
    ...twoSheetProjection,
    state: {
      ...twoSheetProjection.state,
      album: {
        ...twoSheetProjection.state.album,
        sheets: twoSheetProjection.state.album.sheets.map((sheet, index) => ({
          ...sheet,
          pageNumbers: index === 0 ? [7, 8] : [12, 13],
        })),
      },
    },
  };
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithProjectedPageNumbers}
      projectSessionPort={projectSessionPortWithApply(
        async () => projectionWithProjectedPageNumbers,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const gradeTrigger = screen.getByRole("button", {
    name: "Grade de Lâminas",
  });
  expect(
    gradeTrigger.querySelector(".inspector-section-meta"),
  ).toHaveTextContent("2");

  const tiles = Array.from(
    view.container.querySelectorAll<HTMLElement>(".sheet-tile"),
  );
  expect(tiles).toHaveLength(2);
  expect(tiles[0]).toHaveAttribute("aria-current", "true");
  expect(tiles[1]).not.toHaveAttribute("aria-current");
  expect(tiles[0]).toHaveAttribute("data-active-sides", "both");
  expect(tiles[0]).toHaveAccessibleName(
    "Ir para Lâmina 01, Páginas 7–8",
  );
  expect(tiles[0].querySelector(".sheet-tile__number")).toHaveTextContent(
    "01",
  );
  expect(tiles[0].querySelector(".sheet-tile__pages")?.textContent).toBe(
    "7–8",
  );
  expect(tiles[1].querySelector(".sheet-tile__pages")?.textContent).toBe(
    "12–13",
  );
});

test("shows Page numbers instead of cover and final aliases", () => {
  const projectionWithSinglePageEnds: EditorProjection = {
    ...twoSheetProjection,
    state: {
      ...twoSheetProjection.state,
      album: {
        ...twoSheetProjection.state.album,
        sheets: twoSheetProjection.state.album.sheets.map((sheet, index) => ({
          ...sheet,
          activeSides: index === 0 ? "right" : "left",
          pageNumbers: [index + 1],
        })),
      },
    },
    composition: {
      ...twoSheetProjection.composition,
      sheets: twoSheetProjection.composition.sheets.map((sheet, index) => {
        const widthUm = sheet.widthUm / 2;
        return {
          ...sheet,
          activeSides: index === 0 ? "right" : "left",
          widthUm,
          base: {
            ...sheet.base,
            drawRect: { ...sheet.base.drawRect, width: widthUm },
          },
          backgrounds: sheet.backgrounds.map((background) => ({
            ...background,
            drawRect: { ...background.drawRect, width: widthUm },
          })),
          frames: [],
          overlays: [],
        };
      }),
    },
  };
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithSinglePageEnds}
      projectSessionPort={projectSessionPortWithApply(
        async () => projectionWithSinglePageEnds,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const tiles = Array.from(
    view.container.querySelectorAll<HTMLElement>(".sheet-tile"),
  );
  const tilePreviews = tiles.map((tile) =>
    tile.querySelector<HTMLElement>(".sheet-preview-shell"),
  );
  expect(
    tiles.map(
      (tile) => tile.querySelector(".sheet-tile__pages")?.textContent,
    ),
  ).toEqual(["1", "2"]);
  expect(tiles[0]).toHaveAccessibleName(
    "Ir para Lâmina 01, Lâmina inicial, Página 1",
  );
  expect(tiles[1]).toHaveAccessibleName(
    "Ir para Lâmina 02, Lâmina final, Página 2",
  );
  expect(
    tilePreviews[0]?.style.getPropertyValue(
      "--sheet-inactive-side-gradient",
    ),
  ).toBe(
    "linear-gradient(to right, #faf9f6 0%, #ebe3d8 58%, #cec2b2 100%)",
  );
  expect(
    tilePreviews[1]?.style.getPropertyValue(
      "--sheet-inactive-side-gradient",
    ),
  ).toBe(
    "linear-gradient(to left, #faf9f6 0%, #ebe3d8 58%, #cec2b2 100%)",
  );
});

test("uses reduced Cache previews in the media panel and Canvas", () => {
  const mediaPreviewUrls = {
    "media-001": "asset://localhost/cache/media-001.jpg",
  };
  const mediaPreviews = {
    "media-001": {
      mediaId: "media-001",
      state: "ready" as const,
      url: mediaPreviewUrls["media-001"],
    },
  };
  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      mediaPreviews={mediaPreviews}
      onProjectionChange={() => undefined}
    />,
  );

  expect(
    view.container.querySelector<HTMLImageElement>(
      '.media-preview-thumbnail img[src="asset://localhost/cache/media-001.jpg"]',
    ),
  ).not.toBeNull();
  expect(canvasHarness.props?.mediaPreviewUrls).toEqual(
    mediaPreviewUrls,
  );
});

test("offers retry only for an unavailable occurrence and keeps Relink exclusive to absent", async () => {
  const fourStateProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      album: {
        ...projection.state.album,
        media: [
          ...projection.state.album.media,
          {
            ...projection.state.album.media[0],
            id: "media-004",
            name: "Floresta.jpg",
          },
        ],
      },
    },
    mediaUsage: [
      ...projection.mediaUsage,
      { mediaId: "media-004", count: 0 },
    ],
  };
  const relinkedProjection: EditorProjection = {
    ...fourStateProjection,
    state: {
      ...fourStateProjection.state,
      revision: fourStateProjection.state.revision + 1,
      dirty: true,
      canUndo: true,
    },
  };
  const relink = vi.fn<ProjectCorePort["relink"]>(async () =>
    relinkedProjection
  );
  const projectCorePort: ProjectCorePort = {
    ...projectCorePortWithApply(async () => projection),
    relink,
    load: async () => relinkedProjection,
  };
  const onProjectionChange = vi.fn();
  const onRetryUnavailableMedia = vi.fn(async () => undefined);
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={fourStateProjection}
      projectCorePort={projectCorePort}
      mediaPreviews={{
        "media-001": {
          mediaId: "media-001",
          state: "absent",
          url: "asset://localhost/cache/media-001-last.jpg",
        },
        "media-002": {
          mediaId: "media-002",
          state: "unavailable",
          url: null,
        },
        "media-003": {
          mediaId: "media-003",
          state: "cache_unavailable",
          url: "asset://localhost/cache/media-003-last.jpg",
        },
        "media-004": {
          mediaId: "media-004",
          state: "ready",
          url: "asset://localhost/cache/media-004.jpg",
        },
      }}
      onRetryUnavailableMedia={onRetryUnavailableMedia}
      onProjectionChange={onProjectionChange}
    />,
  );

  expect(
    screen.getAllByRole("button", { name: /Religar arquivo de/i }),
  ).toHaveLength(1);
  expect(
    screen.getAllByRole("button", { name: /Tentar novamente o arquivo de/i }),
  ).toHaveLength(1);
  expect(screen.getByRole("status", { name: /Prévia indisponível/i }))
    .toBeInTheDocument();
  const availabilityStatuses = screen.getAllByRole("status", {
    name: /^(Arquivo ausente|Indisponível|Prévia indisponível)/,
  });
  expect(availabilityStatuses).toHaveLength(3);
  expect(screen.getByRole("status", { name: /^Arquivo ausente/ })).toHaveTextContent(/^Ausente$/);
  expect(screen.getByRole("status", { name: "Indisponível" })).toHaveTextContent(/^Indisponível$/);
  expect(screen.getByRole("status", { name: /^Prévia indisponível/ })).toHaveTextContent(/^Prévia indisponível/);
  fireEvent.click(
    screen.getByRole("button", { name: /Religar arquivo de/i }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: /Tentar novamente o arquivo de/i }),
  );

  await waitFor(() => expect(relink).toHaveBeenCalledWith("media-001", expect.any(Function)));
  expect(onRetryUnavailableMedia).toHaveBeenCalledWith("media-002", expect.any(Function));
  expect(onProjectionChange).toHaveBeenLastCalledWith(relinkedProjection);
});

test("merges only Panel viewport and one-row preload margin with Canvas demand", async () => {
  const onMediaDemandChange = vi.fn();
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      onMediaDemandChange={onMediaDemandChange}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onMediaDemandChange?.({
      visibleMediaIds: ["media-003"],
      preloadMediaIds: ["decorative-not-in-panel"],
    });
    emitPanelIntersections("0px", {
      "media-001": true,
      "media-002": false,
      "media-003": false,
    });
    emitPanelIntersections("122px 0px", {
      "media-001": true,
      "media-002": true,
      "media-003": false,
    });
  });

  await waitFor(() =>
    expect(onMediaDemandChange).toHaveBeenLastCalledWith({
      visibleMediaIds: ["media-003", "media-001"],
      preloadMediaIds: ["decorative-not-in-panel", "media-002"],
    }),
  );
});

test("preloads imported Decoratives for Album design before they are used", async () => {
  const onMediaDemandChange = vi.fn();
  const projectionWithUnusedDecorative: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      album: {
        ...projection.state.album,
        media: [
          ...projection.state.album.media,
          {
            id: "decorative-unused",
            kind: "decorative",
            name: "Textura ainda não usada.png",
            palette: ["#E8E1D6", "#B8AA96", "#81705A"],
            sourceHeightPx: 1_200,
            sourceWidthPx: 1_600,
          },
        ],
      },
    },
  };

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithUnusedDecorative}
      projectSessionPort={projectSessionPortWithApply(
        async () => projectionWithUnusedDecorative,
      )}
      onMediaDemandChange={onMediaDemandChange}
      onProjectionChange={() => undefined}
    />,
  );

  await waitFor(() =>
    expect(onMediaDemandChange).toHaveBeenCalledWith({
      visibleMediaIds: [],
      preloadMediaIds: ["decorative-unused"],
    }),
  );
});

test("remeasures Panel demand on card resize and retires obsolete observers", () => {
  const onMediaDemandChange = vi.fn();
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onMediaDemandChange={onMediaDemandChange}
      onProjectionChange={() => undefined}
    />,
  );

  const grid = screen.getByRole("group", { name: "Grade de Fotos" });
  Object.defineProperties(grid, {
    clientWidth: { value: 202 },
    clientHeight: { value: 84 },
  });
  Object.assign(grid.style, { padding: "0px", rowGap: "0px", columnGap: "0px" });
  fireEvent.scroll(grid);
  expect(onMediaDemandChange).toHaveBeenLastCalledWith({
    visibleMediaIds: ["media-002", "media-003"],
    preloadMediaIds: ["media-001"],
  });

  const previousObservers = [...observedViewports];
  const observerCount = observedViewports.length;
  fireEvent.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  expect(observerCount).toBe(2);
  expect(observedViewports).toHaveLength(observerCount);
  fireEvent.change(
    screen.getByRole("slider", { name: "Tamanho das miniaturas" }),
    { target: { value: "124" } },
  );

  expect(onMediaDemandChange).toHaveBeenLastCalledWith({
    visibleMediaIds: ["media-002"],
    preloadMediaIds: ["media-003", "media-001"],
  });
  expect(previousObservers.every(({ targets }) => targets.size === 0)).toBe(true);
  expect(observedViewports.filter(({ targets }) => targets.size > 0)).toHaveLength(2);

  onMediaDemandChange.mockClear();
  act(() => {
    previousObservers.forEach(({ callback }) => callback([], {} as IntersectionObserver));
  });
  expect(onMediaDemandChange).not.toHaveBeenCalled();

  grid.scrollTop = 124;
  fireEvent.scroll(grid);
  expect(onMediaDemandChange).toHaveBeenLastCalledWith({
    visibleMediaIds: ["media-003"],
    preloadMediaIds: ["media-002", "media-001"],
  });
});

test("shares one Decorative Cache preview across Panel, Canvas, and Grade", () => {
  const mediaPreviewUrls = {
    "decorative-overlay": decorativePreviewUrl,
  };
  const mediaPreviews = {
    "decorative-overlay": {
      mediaId: "decorative-overlay",
      state: "ready" as const,
      url: decorativePreviewUrl,
    },
  };
  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={decorativeProjection}
      projectCorePort={projectCorePortWithApply(
        async () => decorativeProjection,
      )}
      mediaPreviews={mediaPreviews}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));

  expect(
    screen.getByRole("button", { name: /^Overlay translúcido\.png/ }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /^Serra ao amanhecer\.jpg/ }),
  ).not.toBeInTheDocument();
  expect(
    view.container.querySelector<HTMLImageElement>(
      `.media-preview-thumbnail img[src="${decorativePreviewUrl}"]`,
    ),
  ).not.toBeNull();
  expect(
    view.container.querySelector<SVGImageElement>(
      `[data-preview-overlay-id="decorative-overlay"][href="${decorativePreviewUrl}"]`,
    ),
  ).not.toBeNull();
  expect(canvasHarness.props?.mediaPreviewUrls).toEqual(mediaPreviewUrls);
});

test("keeps a demanded Decorative pending until its ready Cache preview arrives", () => {
  const projectSessionPort = projectSessionPortWithApply(
    async () => decorativeProjection,
  );
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={decorativeProjection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );
  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );

  expect(
    albumDesign.getByLabelText("Overlay de ambos os lados"),
  ).toHaveAttribute("data-preview-state", "pending");
  expect(view.container.querySelector('image[href=""]')).toBeNull();

  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      mediaPreviews={{
        "decorative-overlay": {
          mediaId: "decorative-overlay",
          state: "ready",
          url: decorativePreviewUrl,
        },
      }}
      projection={decorativeProjection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );

  expect(
    albumDesign.getByLabelText("Overlay de ambos os lados"),
  ).toHaveAttribute("data-preview-state", "ready");
  expect(
    albumDesign.getByLabelText("Overlay de ambos os lados"),
  ).toHaveAttribute("href", decorativePreviewUrl);
});

test.each(["absent", "unavailable"] as const)(
  "preserves a %s Decorative state through the Album Design fallback",
  (state) => {
    const albumDesign = within(
      render(
        <ProjectWorkspace
          exportPort={exportPort}
          mediaPreviews={{
            "decorative-overlay": {
              mediaId: "decorative-overlay",
              state,
              url: null,
            },
          }}
          projection={decorativeProjection}
          projectSessionPort={projectSessionPortWithApply(
            async () => decorativeProjection,
          )}
          onProjectionChange={() => undefined}
        />,
      ).getByLabelText("Prévia do padrão visual do Álbum"),
    );

    expect(
      albumDesign.getByLabelText("Overlay de ambos os lados"),
    ).toHaveAttribute("data-preview-state", state);
  },
);

test("keeps a retained Decorative preview while preserving unavailable state", () => {
  const albumDesign = within(
    render(
      <ProjectWorkspace
        exportPort={exportPort}
        mediaPreviews={{
          "decorative-overlay": {
            mediaId: "decorative-overlay",
            state: "unavailable",
            url: decorativePreviewUrl,
          },
        }}
        projection={decorativeProjection}
        projectSessionPort={projectSessionPortWithApply(
          async () => decorativeProjection,
        )}
        onProjectionChange={() => undefined}
      />,
    ).getByLabelText("Prévia do padrão visual do Álbum"),
  );

  expect(
    albumDesign.getByLabelText("Overlay de ambos os lados"),
  ).toHaveAttribute("data-preview-state", "unavailable");
  expect(
    albumDesign.getByLabelText("Overlay de ambos os lados"),
  ).toHaveAttribute("href", decorativePreviewUrl);
});

test("renders derived media usage as the thumbnail opacity state", () => {
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  const usedMedia = screen.getByRole("button", {
    name: "Serra ao amanhecer.jpg. Já usada. 1 uso",
  });
  expect(usedMedia).toHaveAttribute("data-used", "true");
  expect(usedMedia).not.toHaveTextContent("Serra ao amanhecer.jpg");
});

test("centers a Grade navigation target in the visible Canvas", () => {
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={twoSheetProjection}
      projectCorePort={projectCorePortWithApply(async () => twoSheetProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCanvasMetricsChange?.({
      width: 1_000,
      height: 500,
      scale: 0.5,
    });
  });
  fireEvent.click(screen.getByText("02").closest("button")!);

  const targetCenter =
    canvasHarness.props!.continuousCanvasLayout.entriesAtScale(0.5)[1].center;
  expect(useEditorView.getState().viewport.offsetX).toBeCloseTo(
    1_000 / 2 - targetCenter * 0.5,
  );
});

test("centers the previous and next physical Sheet with ArrowLeft and ArrowRight", () => {
  const physicalProjection = createThreeSheetProjection();
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={physicalProjection}
      projectCorePort={projectCorePortWithApply(async () => physicalProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCanvasMetricsChange?.({
      width: 1_000,
      height: 500,
      scale: 0.5,
    });
  });

  act(() => {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight" }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight" }));
  });
  const thirdCenter =
    canvasHarness.props!.continuousCanvasLayout.entriesAtScale(0.5)[2].center;
  expect(useEditorView.getState()).toMatchObject({
    centeredSheetId: "sheet-003",
    focusedSheetId: "sheet-003",
  });
  expect(useEditorView.getState().viewport.offsetX).toBeCloseTo(
    1_000 / 2 - thirdCenter * 0.5,
  );

  act(() => {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft" }));
  });
  const secondCenter =
    canvasHarness.props!.continuousCanvasLayout.entriesAtScale(0.5)[1].center;
  expect(useEditorView.getState()).toMatchObject({
    centeredSheetId: "sheet-002",
    focusedSheetId: "sheet-002",
  });
  expect(useEditorView.getState().viewport.offsetX).toBeCloseTo(
    1_000 / 2 - secondCenter * 0.5,
  );
});

test("completes Grade navigation requested before Canvas metrics exist", () => {
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={twoSheetProjection}
      projectCorePort={projectCorePortWithApply(async () => twoSheetProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(screen.getByText("02").closest("button")!);
  expect(useEditorView.getState().viewport.offsetX).toBe(42);

  act(() => {
    canvasHarness.props?.onCanvasMetricsChange?.({
      width: 1_000,
      height: 500,
      scale: 0.5,
    });
  });

  const targetCenter =
    canvasHarness.props!.continuousCanvasLayout.entriesAtScale(0.5)[1].center;
  expect(useEditorView.getState().viewport.offsetX).toBeCloseTo(
    1_000 / 2 - targetCenter * 0.5,
  );
  expect(useEditorView.getState().focusedSheetId).toBe("sheet-002");
  expect(useEditorView.getState().centeredSheetId).toBe("sheet-002");
});

test("resizes both workspace panels and persists only completed drags", async () => {
  let persisted = createWorkspacePreferences();
  const update = vi.fn<WorkspacePreferencesPort["update"]>(async (change) => {
    persisted = applyWorkspacePreferenceChange(persisted, change);
    return persisted;
  });
  const workspacePreferencesPort: WorkspacePreferencesPort = {
    load: async () => persisted,
    update,
  };
  const firstView = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      workspacePreferences={{
        kind: "persistent",
        port: workspacePreferencesPort,
      }}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(screen.queryByText("Canvas contÃ­nuo")).not.toBeInTheDocument();

  const verticalSplitter = screen.getByRole("separator", {
    name: "Redimensionar Painel contextual",
  });
  const horizontalSplitter = screen.getByRole("separator", {
    name: "Redimensionar Painel de imagens",
  });
  const workspace = verticalSplitter.parentElement!;
  vi.spyOn(workspace, "getBoundingClientRect").mockReturnValue({
    left: 0,
    right: 1_200,
    top: 0,
    bottom: 800,
    width: 1_200,
    height: 800,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });

  fireEvent.pointerDown(verticalSplitter, { pointerId: 1 });
  fireEvent.pointerMove(window, { clientX: 850, clientY: 0 });
  expect(update).not.toHaveBeenCalled();
  fireEvent.pointerUp(window, { pointerId: 1 });
  await waitFor(() => expect(update).toHaveBeenCalledOnce());

  fireEvent.pointerDown(horizontalSplitter, { pointerId: 2 });
  fireEvent.pointerMove(window, { clientX: 0, clientY: 600 });
  expect(update).toHaveBeenCalledOnce();
  fireEvent.pointerUp(window, { pointerId: 2 });
  await waitFor(() => expect(update).toHaveBeenCalledTimes(2));

  expect(workspace.getAttribute("style")).toContain(
    "--inspector-width: 350px",
  );
  expect(workspace.getAttribute("style")).toContain(
    "--media-panel-height: 200px",
  );
  expect(localStorage.getItem("myalbuns.workspace.inspector-width")).toBeNull();
  expect(localStorage.getItem("myalbuns.workspace.media-panel-height")).toBeNull();

  firstView.unmount();
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      workspacePreferences={{
        kind: "persistent",
        port: workspacePreferencesPort,
      }}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() =>
    expect(
      screen
        .getByRole("separator", {
          name: "Redimensionar Painel contextual",
        })
        .parentElement?.getAttribute("style"),
    ).toContain("--inspector-width: 350px"),
  );
});

test("commits a slider zoom once without flashing a global busy state", async () => {
  const pending = deferredProjection();
  const apply = vi.fn(() => pending.promise);
  useEditorView.setState({ selectedFrameIds: ["frame-001"] });

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const slider = screen.getByRole("slider", { name: "Zoom da Foto" });
  const exportButton = screen.getByRole("button", {
    name: "Exportar Lâmina",
  });

  fireEvent.pointerDown(slider);
  fireEvent.change(slider, { target: { value: "112" } });
  fireEvent.change(slider, { target: { value: "125" } });

  expect(apply).not.toHaveBeenCalled();

  fireEvent.pointerUp(slider);

  expect(apply).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith({
    kind: "transformPhoto",
    frameId: "frame-001",
    deltaPanX: 0,
    deltaPanY: 0,
    deltaZoom: 0.25,
  }, expect.any(Function));
  expect(screen.queryByText("Aplicando alteração")).not.toBeInTheDocument();
  expect(exportButton).toBeEnabled();

  await act(async () => {
    pending.resolve(projection);
    await pending.promise;
  });
});

test("updates the contextual Zoom slider during a Canvas gesture", () => {
  const apply = vi.fn(async () => projection);
  useEditorView.setState({ selectedFrameIds: ["frame-001"] });

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const slider = screen.getByRole("slider", { name: "Zoom da Foto" });
  expect(slider).toHaveValue("100");

  act(() => {
    canvasHarness.props?.onTransformPreview?.({
      frameId: "frame-001",
      panX: 0.35,
      panY: -0.2,
      zoom: 1.25,
    });
  });

  expect(slider).toHaveValue("125");
  expect(screen.getByText("Pan horizontal").parentElement).toHaveTextContent(
    "35%",
  );
  expect(apply).not.toHaveBeenCalled();

  act(() => {
    canvasHarness.props?.onTransformPreview?.(null);
  });

  expect(slider).toHaveValue("100");
  expect(screen.getByText("Pan horizontal").parentElement).toHaveTextContent(
    "0%",
  );
});

test("discards a live Canvas value when its commit fails", async () => {
  const apply = vi.fn(async () => {
    throw new Error("Falha simulada");
  });
  const dialog = projectDialogHarness();
  useEditorView.setState({ selectedFrameIds: ["frame-001"] });

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPortWithApply(apply)}
      projectCorePort={projectCorePortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const slider = screen.getByRole("slider", { name: "Zoom da Foto" });
  act(() => {
    canvasHarness.props?.onTransformPreview?.({
      frameId: "frame-001",
      panX: 0,
      panY: 0,
      zoom: 1.25,
    });
  });
  expect(slider).toHaveValue("125");

  let accepted = true;
  await act(async () => {
    accepted =
      (await canvasHarness.props?.onTransformCommit({
        frameId: "frame-001",
        deltaPanX: 0,
        deltaPanY: 0,
        deltaZoom: 0.25,
      })) ?? true;
  });

  expect(accepted).toBe(false);
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "projectOperationFailure",
      message: "Falha simulada",
    }),
  );
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(slider).toHaveValue("100");
});

test("does not let an old Project completion clear a new slider draft", async () => {
  const pending = deferredProjection();
  const oldApply = vi.fn(() => pending.promise);
  const otherProject = {
    ...projection,
    state: {
      ...projection.state,
      projectId: "project-spike-002",
    },
  };
  const newApply = vi.fn(async () => otherProject);
  const onProjectionChange = vi.fn();
  const oldProjectCorePort = projectCorePortWithApply(oldApply);
  const newProjectCorePort = projectCorePortWithApply(newApply);
  useEditorView.setState({ selectedFrameIds: ["frame-001"] });

  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={oldProjectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );
  const oldSlider = screen.getByRole("slider", {
    name: "Zoom da Foto",
  });
  fireEvent.pointerDown(oldSlider);
  fireEvent.change(oldSlider, { target: { value: "125" } });
  fireEvent.pointerUp(oldSlider);
  expect(oldApply).toHaveBeenCalledOnce();

  view.rerender(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={otherProject}
      projectCorePort={newProjectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );
  act(() => useEditorView.setState({ selectedFrameIds: ["frame-001"] }));
  const newSlider = screen.getByRole("slider", {
    name: "Zoom da Foto",
  });
  fireEvent.pointerDown(newSlider);
  fireEvent.change(newSlider, { target: { value: "130" } });
  expect(newSlider).toHaveValue("130");

  await act(async () => {
    pending.resolve(projection);
    await pending.promise;
  });

  expect(newSlider).toHaveValue("130");
  expect(onProjectionChange).not.toHaveBeenCalled();

  fireEvent.pointerUp(newSlider);
  expect(newApply).toHaveBeenCalledWith({
    kind: "transformPhoto",
    frameId: "frame-001",
    deltaPanX: 0,
    deltaPanY: 0,
    deltaZoom: 0.3,
  }, expect.any(Function));
});

test("uses the Canvas-centered sheet for a media double click", () => {
  const apply = vi.fn(async () => twoSheetProjection);

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={twoSheetProjection}
      projectCorePort={projectCorePortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCenteredSheetChange?.("sheet-002");
  });
  fireEvent.doubleClick(
    screen.getByRole("button", { name: "Campo.jpg" }),
  );

  expect(apply).toHaveBeenCalledWith({
    kind: "addPhoto",
    sheetId: "sheet-002",
    mediaId: "media-002",
    mode: "normal",
  }, expect.any(Function));
});

test("imports a JPEG through the Host boundary without inserting it automatically", async () => {
  const importedProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      revision: projection.state.revision + 1,
      album: {
        ...projection.state.album,
        media: [
          ...projection.state.album.media,
          {
            id: "media-imported",
            kind: "photo",
            name: "Importada.jpg",
            sourceWidthPx: 3_000,
            sourceHeightPx: 2_000,
            palette: ["#111111", "#777777", "#EEEEEE"],
          },
        ],
      },
    },
  };
  const port = projectCorePortWithApply(async () => projection);
  const importMedia = vi.fn(async () => ({
    kind: "completed" as const,
    projection: importedProjection,
    mediaIds: ["media-imported"], importedCount: 1, problems: [],
  }));
  port.importMedia = importMedia;
  const applyWithOutcome = vi.fn(port.applyWithOutcome);
  port.applyWithOutcome = applyWithOutcome;
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={port}
      onProjectionChange={onProjectionChange}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "Importar" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivos…" }));

  await waitFor(() => expect(importMedia).toHaveBeenCalledOnce());
  expect(onProjectionChange).toHaveBeenCalledWith(importedProjection);
  expect(applyWithOutcome).not.toHaveBeenCalled();
});

test.each((["files", "folder"] as const).flatMap((source) =>
  (["completed", "cancelled", "failed"] as const).map((outcome) => ({ source, outcome })),
))("shows $source import progress after selection and releases it on $outcome", async ({ source, outcome }) => {
  const port = projectCorePortWithApply(async () => projection);
  let progress: Parameters<ProjectCorePort["importMedia"]>[0] = () => undefined;
  let resolve!: (result: Awaited<ReturnType<ProjectCorePort["importMedia"]>>) => void;
  let reject!: (error: Error) => void;
  port.importMedia = vi.fn<ProjectCorePort["importMedia"]>((onProgress) => {
    progress = onProgress;
    return new Promise((yes, no) => { resolve = yes; reject = no; });
  });
  const dialogs = projectDialogHarness();
  render(<ProjectWorkspace exportPipelinePort={exportPipelinePort}
    projection={projection} projectCorePort={port} projectDialogPort={dialogs.port}
    onProjectionChange={() => undefined} />);
  fireEvent.click(screen.getByRole("button", { name: "Importar" }));
  fireEvent.click(screen.getByRole("menuitem", { name: source === "files" ? "Arquivos…" : "Pasta…" }));
  await waitFor(() => expect(port.importMedia).toHaveBeenCalledOnce());
  expect(dialogs.present).not.toHaveBeenCalled();
  if (outcome !== "cancelled") {
    act(() => progress({ completedFiles: 0, totalFiles: 0 }));
    await waitFor(() => expect(dialogs.present).toHaveBeenCalledWith({
      kind: "imageProcessingProgress",
      progress: { kind: "indeterminate", status: "Aguarde…" },
    }));
    act(() => progress?.({ completedFiles: 0, totalFiles: 12 }));
    await waitFor(() => expect(dialogs.present).toHaveBeenCalledWith({
      kind: "imageProcessingProgress",
      progress: { kind: "determinate", completed: 0, total: 12, status: "0 de 12" },
    }));
    act(() => progress({ completedFiles: 5, totalFiles: 12 }));
    await waitFor(() => expect(dialogs.present).toHaveBeenLastCalledWith({
      kind: "imageProcessingProgress",
      progress: { kind: "determinate", completed: 5, total: 12, status: "5 de 12" },
    }));
  }
  await act(async () => {
    if (outcome === "failed") reject(new Error("Falha na importação."));
    else if (outcome === "cancelled") resolve({ kind: "cancelled", projection });
    else resolve({ kind: "completed", projection, mediaIds: ["media-002"], importedCount: 12, problems: [] });
  });
  if (outcome === "cancelled") expect(dialogs.present).not.toHaveBeenCalled();
  else {
    await waitFor(() => expect(dialogs.dismiss).toHaveBeenCalled());
    if (outcome === "completed") {
      expect(dialogs.present).toHaveBeenCalledTimes(3);
      expect(screen.queryByText("12 Fotos importadas.")).not.toBeInTheDocument();
    } else {
      expect(dialogs.present).toHaveBeenLastCalledWith({ kind: "projectOperationFailure", message: "Falha na importação." });
      expect(dialogs.dismiss.mock.invocationCallOrder[0]).toBeLessThan(dialogs.present.mock.invocationCallOrder[dialogs.present.mock.calls.length - 1]);
    }
  }
});

test.each([0, 12])("completes import with %i new Photos without a success dialog or toolbar status text", async (importedCount) => {
  const port = projectCorePortWithApply(async () => projection);
  port.importMedia = vi.fn(async () => ({
    kind: "completed" as const,
    projection,
    mediaIds: ["media-002"], importedCount, problems: [],
  }));
  const dialogs = projectDialogHarness();
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={port}
      projectDialogPort={dialogs.port}
      onProjectionChange={() => undefined}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Importar" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivos…" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Importar" })).toBeEnabled());
  expect(port.importMedia).toHaveBeenCalledOnce();
  expect(dialogs.present).not.toHaveBeenCalled();
  expect(screen.queryByText("12 Fotos importadas.")).not.toBeInTheDocument();
});

test("the connected media panel preserves a group and its anchor through ordering and filters", () => {
  const port = projectCorePortWithApply(async () => projection);
  const apply = vi.spyOn(port, "applyWithOutcome");
  render(<ProjectWorkspace exportPipelinePort={exportPipelinePort} projection={projection}
    projectCorePort={port} onProjectionChange={vi.fn()} />);
  const first = screen.getByRole("button", { name: "Campo.jpg" });
  const second = screen.getByRole("button", { name: "Serra ao amanhecer.jpg. Já usada. 1 uso" });
  fireEvent.click(first);
  fireEvent.click(second, { ctrlKey: true });
  expect(first).toHaveAttribute("aria-pressed", "true");
  expect(second).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(screen.getByRole("button", { name: "Filtro, ordem e tamanho" }));
  fireEvent.change(screen.getByRole("combobox", { name: "Ordenar por" }), { target: { value: "name-descending" } });
  expect(first).toHaveAttribute("aria-pressed", "true");
  expect(second).toHaveAttribute("aria-pressed", "true");
  const search = screen.getByRole("searchbox", { name: "Buscar Fotos" });
  fireEvent.change(search, { target: { value: "campo" } });
  expect(first).toHaveAttribute("aria-pressed", "true");
  fireEvent.change(search, { target: { value: "" } });
  expect(screen.getByRole("button", { name: "Serra ao amanhecer.jpg. Já usada. 1 uso" })).toHaveAttribute("aria-pressed", "false");
  expect(apply).not.toHaveBeenCalled();
});

test("hiding the media panel preserves the search and selection for the open window", () => {
  render(<ProjectWorkspace exportPipelinePort={exportPipelinePort} projection={projection}
    projectCorePort={projectCorePortWithApply(async () => projection)} onProjectionChange={vi.fn()} />);
  fireEvent.change(screen.getByRole("searchbox", { name: "Buscar Fotos" }), { target: { value: "Campo" } });
  fireEvent.click(screen.getByRole("button", { name: "Campo.jpg" }));
  fireEvent.click(getApplicationCommand("Exibir", "Painel de imagens"));
  expect(screen.queryByRole("region", { name: "Painel de imagens" })).not.toBeInTheDocument();
  fireEvent.click(getApplicationCommand("Exibir", "Painel de imagens"));
  expect(screen.getByRole("searchbox", { name: "Buscar Fotos" })).toHaveValue("Campo");
  expect(screen.getByRole("button", { name: "Campo.jpg" })).toHaveAttribute("aria-pressed", "true");
});

test("restores the active media tab in a new window without restoring its search", async () => {
  const preferences = createFallbackWorkspacePreferencesPort();
  const props = { exportPipelinePort, projection,
    projectCorePort: projectCorePortWithApply(async () => projection),
    onProjectionChange: vi.fn(), workspacePreferences: { kind: "persistent" as const, port: preferences } };
  const first = render(<ProjectWorkspace {...props} />);
  fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));
  fireEvent.change(screen.getByRole("searchbox", { name: "Buscar Decorativos" }), { target: { value: "dourado" } });
  await act(async () => {});
  first.unmount();
  render(<ProjectWorkspace {...props} />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Decorativos" })).toHaveAttribute("aria-pressed", "true"));
  expect(screen.getByRole("searchbox", { name: "Buscar Decorativos" })).toHaveValue("");
  expect(props.onProjectionChange).not.toHaveBeenCalled();
});

test("the album absence notice opens a temporary view and restores the previous tab and filters", async () => {
  render(<ProjectWorkspace exportPipelinePort={exportPipelinePort} projection={projection}
    projectCorePort={projectCorePortWithApply(async () => projection)} onProjectionChange={vi.fn()}
    mediaFiles={{ "media-002": { mediaId: "media-002", state: "absent", createdAtMs: null, modifiedAtMs: null } }} />);
  fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));
  fireEvent.change(screen.getByRole("searchbox", { name: "Buscar Decorativos" }), { target: { value: "dourado" } });
  fireEvent.click(screen.getByRole("button", { name: "Filtro, ordem e tamanho" }));
  fireEvent.change(screen.getByRole("combobox", { name: "Filtro de uso" }), { target: { value: "used" } });
  fireEvent.click(screen.getByRole("button", { name: "Ver arquivos ausentes" }));
  expect(screen.getByRole("button", { name: "Fotos" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Campo.jpg. Arquivo ausente" })).toBeVisible();
  expect(screen.queryByRole("button", { name: "Serra ao amanhecer.jpg. Já usada. 1 uso" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Encerrar visualização de ausentes" }));
  expect(screen.getByRole("button", { name: "Decorativos" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("searchbox", { name: "Buscar Decorativos" })).toHaveValue("dourado");
  fireEvent.click(screen.getByRole("button", { name: "Filtro, ordem e tamanho" }));
  expect(screen.getByRole("combobox", { name: "Filtro de uso" })).toHaveValue("used");
});

test("reimporting a JPEG selects its existing card without a creative mutation", async () => {
  const port = projectCorePortWithApply(async () => projection);
  const importMedia = vi.fn(async () => ({
    kind: "completed" as const,
    projection,
    mediaIds: ["media-002"], importedCount: 0, problems: [],
  }));
  port.importMedia = importMedia;
  const applyWithOutcome = vi.fn(port.applyWithOutcome);
  port.applyWithOutcome = applyWithOutcome;
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={port}
      onProjectionChange={onProjectionChange}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "Importar" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivos…" }));

  await waitFor(() => expect(importMedia).toHaveBeenCalledOnce());
  const existingPhoto = screen.getByRole("button", { name: "Campo.jpg" });
  await waitFor(() =>
    expect(existingPhoto).toHaveAttribute("aria-pressed", "true"),
  );
  expect(onProjectionChange).toHaveBeenCalledWith(projection);
  expect(applyWithOutcome).not.toHaveBeenCalled();
});

test("resolves a mode-free target while dropping a Photo in the current Canvas mode", async () => {
  const port = projectCorePortWithApply(async () => projection);
  const resolvePhotoDropTarget = vi.fn(async () => ({
    kind: "frame" as const,
    frameId: "frame-001",
  }));
  const applyWithOutcome = vi.fn(async () => ({
    projection,
    affectedFrameId: "frame-001",
    affectedSheetId: null,
  }));
  port.resolvePhotoDropTarget = resolvePhotoDropTarget;
  port.applyWithOutcome = applyWithOutcome;

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={port}
      onProjectionChange={() => undefined}
    />,
  );
  const point = { sheetId: "sheet-001", xUm: 25_000, yUm: 30_000 };

  await expect(
    canvasHarness.props?.onResolvePhotoDropTarget?.("media-002", point),
  ).resolves.toEqual({ kind: "frame", frameId: "frame-001" });
  await act(async () => {
    await canvasHarness.props?.onDropPhoto?.("media-002", point);
  });

  expect(resolvePhotoDropTarget).toHaveBeenCalledWith(
    "sheet-001",
    25_000,
    30_000,
  );
  expect(applyWithOutcome).toHaveBeenCalledWith({
    kind: "dropPhoto",
    sheetId: "sheet-001",
    mediaId: "media-002",
    xUm: 25_000,
    yUm: 30_000,
    mode: "normal",
  }, expect.any(Function));
  expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-001"]);

  act(() => canvasHarness.props?.onEditSheet?.("sheet-001"));
  await canvasHarness.props?.onResolvePhotoDropTarget?.("media-002", point);
  expect(resolvePhotoDropTarget).toHaveBeenLastCalledWith(
    "sheet-001",
    25_000,
    30_000,
  );
  expect(resolvePhotoDropTarget).toHaveBeenCalledTimes(2);
});

test.each(["Delete", "context menu"])("removes the selected Photos through one consolidated decision from %s", async (source) => {
  const dialog = projectDialogHarness();
  const apply = vi.fn(async () => projection);
  const input = { ...projection, mediaUsage: projection.mediaUsage.map((usage) => ({ ...usage, count: 1 })) };
  render(<ProjectWorkspace exportPipelinePort={exportPipelinePort} projection={input}
    projectDialogPort={dialog.port} projectCorePort={projectCorePortWithApply(apply)} onProjectionChange={() => undefined} />);
  const panel = screen.getByRole("region", { name: "Painel de imagens" });
  const photos = within(panel).getAllByRole("button").filter((button) => button.hasAttribute("data-media-id"));
  fireEvent.click(photos[0]);
  fireEvent.click(photos[1], { ctrlKey: true });
  const ids = [photos[0].getAttribute("data-media-id"), photos[1].getAttribute("data-media-id")];
  if (source === "Delete") fireEvent.keyDown(photos[1], { key: "Delete" });
  else {
    fireEvent.contextMenu(photos[0], { clientX: 80, clientY: 500 });
    fireEvent.click(screen.getByRole("menuitem", { name: "Remover" }));
  }
  await waitFor(() => expect(dialog.present).toHaveBeenCalledWith(expect.objectContaining({ kind: "mediaRemovalConfirmation", count: 2, usedCount: 2, busy: false })));
  expect(apply).not.toHaveBeenCalled();
  await act(async () => { dialog.emit("removeMediaKeepFrames"); });
  await waitFor(() => expect(apply).toHaveBeenCalledOnce());
  expect(apply).toHaveBeenCalledWith({ kind: "removeMedia", mediaIds: ids, mode: "keepFrames" });
  expect(dialog.dismiss).toHaveBeenCalledOnce();
});

test("Delete respects text focus and an unused Photo selection is removed directly once", async () => {
  const dialog = projectDialogHarness();
  const apply = vi.fn(async () => projection);
  const input = { ...projection, mediaUsage: projection.mediaUsage.map((usage) => ({ ...usage, count: 0 })) };
  render(<ProjectWorkspace exportPipelinePort={exportPipelinePort} projection={input}
    projectDialogPort={dialog.port} projectCorePort={projectCorePortWithApply(apply)} onProjectionChange={() => undefined} />);
  const panel = screen.getByRole("region", { name: "Painel de imagens" });
  const photos = within(panel).getAllByRole("button").filter((button) => button.hasAttribute("data-media-id"));
  fireEvent.click(photos[0]);
  fireEvent.keyDown(screen.getByRole("searchbox", { name: "Buscar Fotos" }), { key: "Delete" });
  fireEvent.keyDown(document.body, { key: "Delete" });
  expect(apply).not.toHaveBeenCalled();
  fireEvent.contextMenu(photos[1], { clientX: 80, clientY: 500 });
  fireEvent.click(screen.getByRole("menuitem", { name: "Remover" }));
  await waitFor(() => expect(apply).toHaveBeenCalledOnce());
  expect(apply).toHaveBeenCalledWith({ kind: "removeMedia", mediaIds: [photos[1].getAttribute("data-media-id")], mode: "removeAll" });
  expect(dialog.present).not.toHaveBeenCalled();
});

test("starts a pointer drag for the directly pressed Photo and cancels it with Escape", async () => {
  render(<ProjectWorkspace exportPipelinePort={exportPipelinePort} projection={projection}
    projectCorePort={projectCorePortWithApply(async () => projection)} onProjectionChange={() => undefined} />);
  await act(async () => {});
  const photo = screen.getByRole("button", { name: "Campo.jpg" });
  fireEvent.pointerDown(photo, { button: 0, pointerId: 1, clientX: 10, clientY: 10 });
  fireEvent.pointerMove(document, { pointerId: 1, clientX: 40, clientY: 30 });
  expect(canvasHarness.props?.draggedPhotoId).toBe("media-002");
  fireEvent.keyDown(window, { key: "Escape" });
  expect(canvasHarness.props?.draggedPhotoId).toBeNull();
});
test("starts Exportação for the Canvas-centered Lâmina even while focus remains on another Lâmina", () => {
  const startSheet = vi.fn<ExportPipelinePort["startSheet"]>(() => ({
    completion: Promise.resolve({
      status: "completed",
      result: { widthPx: 600, heightPx: 300 },
    }),
    cancel: async () => "not_found",
  }));

  render(
    <ProjectWorkspace
      exportPipelinePort={{ startSheet }}
      projection={twoSheetProjection}
      projectCorePort={projectCorePortWithApply(async () =>
        twoSheetProjection
      )}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCenteredSheetChange?.("sheet-002");
  });
  expect(useEditorView.getState().focusedSheetId).toBe("sheet-001");
  fireEvent.click(
    screen.getByRole("button", { name: "Exportar Lâmina" }),
  );

  expect(startSheet).toHaveBeenCalledWith(
    {
      projectName: "Álbum Horizonte",
      sheetId: "sheet-002",
      sheetNumber: 2,
    },
    expect.any(Function),
  );
});

test("forwards simultaneous Canvas Pan and Zoom as one intent", () => {
  const apply = vi.fn(async () => projection);

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  canvasHarness.props?.onTransformCommit({
    frameId: "frame-001",
    deltaPanX: 0.35,
    deltaPanY: -0.2,
    deltaZoom: 0.12,
  });

  expect(apply).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith({
    kind: "transformPhoto",
    frameId: "frame-001",
    deltaPanX: 0.35,
    deltaPanY: -0.2,
    deltaZoom: 0.12,
  }, expect.any(Function));
});

test("serializes Project mutations so projections cannot arrive out of order", async () => {
  const first = deferredProjection();
  const second = deferredProjection();
  const apply = vi
    .fn<ProjectCorePort["apply"]>()
    .mockImplementationOnce(() => first.promise)
    .mockImplementationOnce(() => second.promise);
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(apply)}
      onProjectionChange={onProjectionChange}
    />,
  );

  canvasHarness.props?.onTransformCommit({
    frameId: "frame-001",
    deltaPanX: 0.1,
    deltaPanY: 0,
    deltaZoom: 0,
  });
  canvasHarness.props?.onTransformCommit({
    frameId: "frame-001",
    deltaPanX: 0.2,
    deltaPanY: 0,
    deltaZoom: 0,
  });

  await act(async () => {
    await Promise.resolve();
  });
  expect(apply).toHaveBeenCalledOnce();

  const firstProjection = {
    ...projection,
    state: { ...projection.state, revision: 26 },
  };
  await act(async () => {
    first.resolve(firstProjection);
    await first.promise;
  });

  expect(onProjectionChange).toHaveBeenLastCalledWith(firstProjection);
  expect(apply).toHaveBeenCalledTimes(2);

  const secondProjection = {
    ...projection,
    state: { ...projection.state, revision: 27 },
  };
  await act(async () => {
    second.resolve(secondProjection);
    await second.promise;
  });

  expect(onProjectionChange).toHaveBeenLastCalledWith(secondProjection);
});


test.each([0, 2])("presents photo import rejections after committing %i valid files", async (importedCount) => {
  const dialog = projectDialogHarness();
  const port = projectCorePortWithApply(async () => projection);
  const problems = [{ fileName: "corrompida.jpg", reason: "JPEG corrompido" }];
  port.importMedia = vi.fn(async () => ({
    kind: "completed" as const, projection, mediaIds: importedCount ? ["media-002"] : [],
    importedCount, problems,
  }));
  const onProjectionChange = vi.fn();
  render(<ProjectWorkspace exportPipelinePort={exportPipelinePort} projection={projection}
    projectCorePort={port} projectDialogPort={dialog.port} onProjectionChange={onProjectionChange} />);
  fireEvent.click(screen.getByRole("button", { name: "Importar" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivos…" }));
  await waitFor(() => expect(dialog.present).toHaveBeenCalledWith({
    kind: "imageProcessingProblems", importedCount, problems,
  }));
  expect(onProjectionChange).toHaveBeenCalledExactlyOnceWith(projection);
  act(() => dialog.emit("dismissImageProcessingProblems"));
  await waitFor(() => expect(dialog.dismiss).toHaveBeenCalled());
  expect(onProjectionChange).toHaveBeenCalledOnce();
});


test.each([
  { problems: [{ fileName: "corrompida.jpg", reason: "JPEG corrompido" }] },
  { problems: [], operationProblem: "Não foi possível continuar o processamento por falta de memória. Tente novamente mais tarde." },
])("preserves partial import feedback after dismissing a queued Save failure: %j", async (feedback) => {
  type ImportResult = Awaited<ReturnType<ProjectCorePort["importMedia"]>>;
  let resolveImport!: (value: ImportResult) => void;
  const pendingImport = new Promise<ImportResult>((resolve) => { resolveImport = resolve; });
  const dialog = projectDialogHarness();
  const port = projectCorePortWithApply(async () => projection);
  port.importMedia = vi.fn(() => pendingImport);
  port.save = vi.fn(async () => { throw new Error("Não foi possível salvar"); });
  const onProjectionChange = vi.fn();
  render(<ProjectWorkspace exportPipelinePort={exportPipelinePort} projection={projection}
    projectCorePort={port} projectDialogPort={dialog.port} onProjectionChange={onProjectionChange} />);
  fireEvent.click(screen.getByRole("button", { name: "Importar" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivos…" }));
  fireEvent.keyDown(window, { ctrlKey: true, key: "s" });
  expect(port.save).not.toHaveBeenCalled();
  await act(async () => {
    resolveImport({ kind: "completed", projection, mediaIds: ["media-002"], importedCount: 1, ...feedback });
    await pendingImport;
  });
  await waitFor(() => expect(dialog.present).toHaveBeenLastCalledWith({
    kind: "projectOperationFailure", message: "Não foi possível salvar",
  }));
  act(() => dialog.emit("dismissProjectOperationFailure"));
  await waitFor(() => expect(dialog.present).toHaveBeenLastCalledWith({
    kind: "imageProcessingProblems", importedCount: 1, ...feedback,
  }));
  act(() => dialog.emit("dismissImageProcessingProblems"));
  await waitFor(() => expect(dialog.dismiss).toHaveBeenCalled());
  expect(onProjectionChange).toHaveBeenCalledExactlyOnceWith(projection);
  expect(port.save).toHaveBeenCalledOnce();
});
