import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import type {
  ExportPipelinePort,
  MediaPreviewDemand,
  ProjectCorePort,
  ProjectWindowPort,
} from "../../application/projectPorts";
import type { ProjectDialogPort } from "../../application/projectDialogPort";
import type { EditorProjection } from "../../domain/project";
import { useEditorView } from "../../state/editorView";
import { emptyLayoutCatalogPort } from "../../test/layoutCatalogPorts";
import { rasterLimitsAt300Dpi } from "../../test/projectConfigurationFixtures";
import { createTwoSheetProjection } from "../../test/projectFixtures";
import type { AlbumCanvasProps } from "../canvas/albumCanvasContract";
import { ProjectWorkspace } from "./ProjectWorkspace";

const canvasHarness = vi.hoisted(() => ({
  props: null as AlbumCanvasProps | null,
}));

vi.mock("../canvas/AlbumCanvas", () => ({
  AlbumCanvas: (props: AlbumCanvasProps) => {
    canvasHarness.props = props;
    return <div data-testid="album-canvas" />;
  },
}));

vi.mock("../inspector/AlbumInformationForm", () => ({
  AlbumInformationForm: () => <div>Conteúdo de Informações do álbum</div>,
}));

vi.mock("../inspector/AlbumDesignForm", () => ({
  AlbumDesignForm: () => <div>Conteúdo de Design do álbum</div>,
}));

const projection: EditorProjection = createTwoSheetProjection();

const projectCorePort: ProjectCorePort = {
  load: async () => projection,
  validateAlbumInformation: async () => ({
    rasterLimits: rasterLimitsAt300Dpi,
    errors: [],
    impact: { conversionLosses: [], heightPx: 3_543, pageWidthPx: 3_543, sheetWidthPx: 7_087 },
  }),
  apply: async () => projection,
  applyWithOutcome: async () => ({ projection, affectedFrameId: null, affectedSheetId: null }),
  importMedia: async () => ({ kind: "cancelled", projection }),
  ...emptyLayoutCatalogPort,
  readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
  readSliderDoubleClickTime: async () => 500,
  validateMediaFolderName: async () => { throw new Error("Folder validation is not configured in this fixture."); },
  queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
  previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
  previewFrameStyle: async () => { throw new Error("Quadro style preview is not configured in this fixture."); },
  previewDecorativeDrop: async () => { throw new Error("Decorative preview is not configured in this fixture."); },
  previewPhotoZoom: async () => { throw new Error("Photo zoom preview is not configured in this fixture."); },
  previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
  previewFrameGeometry: async () => { throw new Error("Quadro geometry preview is not configured in this fixture."); },
  resolvePhotoDropTarget: async () => ({ kind: "invalid" }),
  replaceImage: async () => projection,
  relink: async () => projection,
  undo: async () => projection,
  redo: async () => projection,
  save: async () => ({ outcome: { kind: "alreadyCurrent", revision: projection.state.revision }, projection }),
  saveAs: async () => ({ outcome: { kind: "cancelled" }, projection }),
};

const exportPipelinePort: ExportPipelinePort = {
  defaultDestination: async () => "C:/Exportados/Album",
  chooseDestination: async () => null,
  startSheet: () => ({
    cancel: async () => "not_found",
    completion: Promise.resolve({ status: "completed", result: { widthPx: 1, heightPx: 1 } }),
  }),
};

const projectDialogPort: ProjectDialogPort = {
  acquire: () => ({ dismiss: async () => undefined, present: async () => undefined }),
};

const projectWindowPort: ProjectWindowPort = {
  onCloseRequested: async () => () => undefined,
  requestClose: async () => ({ kind: "closed" }),
  resolveClose: async () => ({ kind: "closed" }),
};

beforeEach(() => {
  canvasHarness.props = null;
  useEditorView.setState({
    projectId: projection.state.projectId,
    selectedFrameIds: [],
    selectedSheetIds: ["sheet-001"],
    sheetSelectionAnchorId: "sheet-001",
    sheetSelectionSource: "focus",
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    editingSheetId: null,
    viewport: { offsetX: 0 },
    inspectorSubject: "canvas",
  });
});

function renderWorkspace(onMediaDemandChange: (demand: MediaPreviewDemand) => void = () => undefined) {
  return render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projectDialogPort={projectDialogPort}
      projectCorePort={projectCorePort}
      projectWindowPort={projectWindowPort}
      projection={projection}
      mediaPreviews={{
        "media-002": { mediaId: "media-002", state: "ready", url: "data:image/png;base64,Q0FNUE8=" },
      }}
      onGraphicsUnavailable={() => undefined}
      onMediaDemandChange={onMediaDemandChange}
      onRetryUnavailableMedia={async () => undefined}
      onPreferencesReady={() => undefined}
      workspacePreferences={{ kind: "memory" }}
      runProjectMutation={{
        run: async () => ({ status: "obsolete" }),
        waitForIdle: async () => null,
      }}
      onProjectionChange={() => undefined}
    />,
  );
}

function thumbnail(name: RegExp) {
  return within(screen.getByRole("region", { name: "Painel de imagens" })).getByRole("button", { name });
}

function contextualPanel() {
  return within(screen.getByRole("complementary", { name: "Painel contextual" }));
}

test("the last explicit selection owns the contextual panel while the Frame stays selected", () => {
  const demand = vi.fn<(demand: MediaPreviewDemand) => void>();
  renderWorkspace(demand);
  expect(contextualPanel().getByRole("button", { name: "Informações do álbum" })).toBeInTheDocument();

  fireEvent.click(thumbnail(/^Campo\.jpg/));
  expect(contextualPanel().getByText("Imagem selecionada")).toBeInTheDocument();
  expect(contextualPanel().getByRole("heading", { name: "Campo.jpg" })).toBeInTheDocument();
  expect(contextualPanel().getByRole("img", { name: "Prévia de Campo.jpg" })).toBeInTheDocument();
  expect(document.querySelector(".inspector-image-preview")!.textContent).toBe("");
  expect(contextualPanel().queryByRole("button", { name: "Grade de lâminas" })).not.toBeInTheDocument();
  expect(demand).toHaveBeenLastCalledWith(expect.objectContaining({
    visibleMediaIds: expect.arrayContaining(["media-002"]),
  }));

  // An explicit Frame selection takes the panel back.
  act(() => canvasHarness.props?.onSelectFrame("frame-001"));
  expect(contextualPanel().getByText("Quadro selecionado")).toBeInTheDocument();
  expect(contextualPanel().getByRole("img", { name: "Prévia de Serra ao amanhecer.jpg" })).toBeInTheDocument();
  expect(thumbnail(/^Campo\.jpg/)).toHaveAttribute("aria-pressed", "true");
  expect(demand).toHaveBeenLastCalledWith(expect.objectContaining({
    visibleMediaIds: expect.arrayContaining(["media-001"]),
  }));

  // A thumbnail claims it again; the Frame stays selected for its shortcuts.
  fireEvent.click(thumbnail(/^Campo\.jpg/));
  expect(contextualPanel().getByRole("heading", { name: "Campo.jpg" })).toBeInTheDocument();
  expect(canvasHarness.props?.selectedFrameIds).toEqual(["frame-001"]);

  // Several images show only their count.
  fireEvent.click(thumbnail(/^Praia\.jpg/), { ctrlKey: true });
  expect(contextualPanel().getByRole("heading", { name: "2 imagens selecionadas" })).toBeInTheDocument();
  expect(contextualPanel().queryByRole("img")).not.toBeInTheDocument();

  // A tap on a Sheet clears the Frames: the Album returns although the
  // images are still selected in the media panel.
  act(() => canvasHarness.props?.onSelectFrame(null));
  expect(contextualPanel().getByRole("button", { name: "Informações do álbum" })).toBeInTheDocument();
  expect(thumbnail(/^Campo\.jpg/)).toHaveAttribute("aria-pressed", "true");
});

test("a selected image hides the Album without rebuilding the Sheet Grid", () => {
  renderWorkspace();
  const grid = contextualPanel().getByTestId("sheet-reorder-grid");

  // New tiles would load their Cache previews again and fill in one by one.
  for (const clear of [
    () => fireEvent.click(screen.getByRole("group", { name: "Grade de fotos" })),
    () => act(() => canvasHarness.props?.onSelectFrame(null)),
  ]) {
    fireEvent.click(thumbnail(/^Campo\.jpg/));
    expect(contextualPanel().getByRole("heading", { name: "Campo.jpg" })).toBeInTheDocument();
    expect(contextualPanel().queryByRole("button", { name: "Grade de lâminas" })).not.toBeInTheDocument();

    clear();
    expect(contextualPanel().getByRole("button", { name: "Grade de lâminas" })).toBeVisible();
    expect(contextualPanel().getByTestId("sheet-reorder-grid")).toBe(grid);
  }
});

test("recentering another Sheet does not take the panel from the selected image", () => {
  renderWorkspace();
  act(() => canvasHarness.props?.onSelectFrame("frame-001"));
  fireEvent.click(thumbnail(/^Praia\.jpg/));
  expect(contextualPanel().getByRole("heading", { name: "Praia.jpg" })).toBeInTheDocument();

  act(() => canvasHarness.props?.onCenteredSheetChange?.("sheet-002"));
  expect(canvasHarness.props?.selectedFrameIds).toEqual([]);
  expect(contextualPanel().getByRole("heading", { name: "Praia.jpg" })).toBeInTheDocument();

  // Clearing the media selection hands the panel back to the canvas.
  fireEvent.click(screen.getByRole("group", { name: "Grade de fotos" }));
  expect(contextualPanel().getByRole("button", { name: "Informações do álbum" })).toBeInTheDocument();
});

test("while a Sheet is edited, the Frame menu returns the panel to the selected Frame", () => {
  renderWorkspace();
  act(() => canvasHarness.props?.onEditSheet?.("sheet-001"));
  act(() => canvasHarness.props?.onSelectFrame("frame-001"));
  fireEvent.click(thumbnail(/^Campo\.jpg/));
  expect(contextualPanel().getByRole("heading", { name: "Campo.jpg" })).toBeInTheDocument();
  expect(contextualPanel().queryByRole("button", { name: "Design da lâmina" })).not.toBeInTheDocument();

  act(() => canvasHarness.props?.onOpenFrameContextMenu?.("frame-001", { x: 40, y: 40 }));
  expect(contextualPanel().getByText("Quadro selecionado")).toBeInTheDocument();
  expect(canvasHarness.props?.selectedFrameIds).toEqual(["frame-001"]);

  fireEvent.keyDown(document.body, { key: "Escape" });
  fireEvent.click(thumbnail(/^Campo\.jpg/));
  act(() => canvasHarness.props?.onSelectFrame(null));
  expect(contextualPanel().getByRole("button", { name: "Design da lâmina" })).toBeInTheDocument();
});

test("a hidden media panel cannot own the contextual panel", () => {
  renderWorkspace();
  fireEvent.click(thumbnail(/^Campo\.jpg/));
  expect(contextualPanel().getByRole("heading", { name: "Campo.jpg" })).toBeInTheDocument();

  fireEvent.click(screen.getByRole("menuitem", { name: "Exibir" }));
  fireEvent.click(screen.getByRole("menuitemcheckbox", { name: "Painel de imagens" }));
  expect(screen.queryByRole("region", { name: "Painel de imagens" })).not.toBeInTheDocument();
  expect(contextualPanel().getByRole("button", { name: "Informações do álbum" })).toBeInTheDocument();
});
