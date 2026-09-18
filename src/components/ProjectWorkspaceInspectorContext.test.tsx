import { rasterLimitsAt300Dpi } from "../test/projectConfigurationFixtures";
import { emptyLayoutCatalogPort } from "../test/layoutCatalogPorts";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type {
  ExportPipelinePort,
  ProjectCorePort,
  ProjectWindowPort,
} from "../application/projectPorts";
import type { ProjectDialogPort } from "../application/projectDialogPort";
import { representativeProjection } from "../test/projectFixtures";
import type { AlbumCanvasProps } from "./albumCanvasContract";
import { ProjectWorkspace } from "./ProjectWorkspace";

const canvasHarness = vi.hoisted(() => ({
  props: null as AlbumCanvasProps | null,
}));

vi.mock("./AlbumCanvas", () => ({
  AlbumCanvas: (props: AlbumCanvasProps) => {
    canvasHarness.props = props;
    return <div data-testid="album-canvas" />;
  },
}));

vi.mock("./MediaPanel", () => ({
  MediaPanel: () => <div data-testid="media-panel" />,
}));

vi.mock("./AlbumInformationForm", () => ({
  AlbumInformationForm: () => <div>Conteúdo de Informações do álbum</div>,
}));

vi.mock("./AlbumDesignForm", () => ({
  AlbumDesignForm: () => <div>Conteúdo de Design do álbum</div>,
}));

const exportPipelinePort: ExportPipelinePort = {
  defaultDestination: async () => "C:/Exportados/Album", chooseDestination: async () => null, startSheet: () => ({
    cancel: async () => "not_found",
    completion: Promise.resolve({
      status: "completed",
      result: { widthPx: 1, heightPx: 1 },
    }),
  }),
};

const projectCorePort: ProjectCorePort = {
  load: async () => representativeProjection,
  validateAlbumInformation: async () => ({ rasterLimits: rasterLimitsAt300Dpi,
    errors: [],
    impact: { conversionLosses: [], heightPx: 3_543, pageWidthPx: 3_543, sheetWidthPx: 7_087 },
  }),
  apply: async () => representativeProjection,
  applyWithOutcome: async () => ({
    projection: representativeProjection,
    affectedFrameId: null,
    affectedSheetId: null,
  }),
  importMedia: async () => ({
    kind: "cancelled",
    projection: representativeProjection,
  }),
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
  replaceImage: async () => representativeProjection, relink: async () => representativeProjection,
  undo: async () => representativeProjection,
  redo: async () => representativeProjection,
  save: async () => ({
    outcome: {
      kind: "alreadyCurrent",
      revision: representativeProjection.state.revision,
    },
    projection: representativeProjection,
  }),
  saveAs: async () => ({
    outcome: { kind: "cancelled" },
    projection: representativeProjection,
  }),
};

const projectDialogPort: ProjectDialogPort = {
  acquire: () => ({
    dismiss: async () => undefined,
    present: async () => undefined,
  }),
};

const projectWindowPort: ProjectWindowPort = {
  onCloseRequested: async () => () => undefined,
  requestClose: async () => ({ kind: "closed" }),
  resolveClose: async () => ({ kind: "closed" }),
};

test("derives Album, Sheet and Frame Inspector contexts from the editing state", async () => {
  const projection = structuredClone(representativeProjection);
  projection.state.album.sheets[0].frames.push({
    ...projection.state.album.sheets[0].frames[0], id: "frame-002", photo: null,
  });
  projection.composition.sheets[0].frames.push({
    ...projection.composition.sheets[0].frames[0], frameId: "frame-002", photo: null,
  });
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projectDialogPort={projectDialogPort}
      projectCorePort={projectCorePort}
      projectWindowPort={projectWindowPort}
      projection={projection}
      mediaPreviews={{}}
      onGraphicsUnavailable={() => undefined}
      onMediaDemandChange={() => undefined}
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

  expect(
    screen.getByRole("button", { name: "Informações do álbum" }),
  ).toBeInTheDocument();

  act(() => canvasHarness.props?.onEditSheet?.("sheet-001"));
  expect(
    screen.getByRole("button", { name: "Design da lâmina" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Informações do álbum" }),
  ).not.toBeInTheDocument();

  act(() => canvasHarness.props?.onSelectFrame("frame-001"));
  expect(screen.getByText("Quadro selecionado")).toBeInTheDocument();

  act(() => canvasHarness.props?.onSelectFrame("frame-002", true));
  expect(screen.getByRole("heading", { name: "2 quadros selecionados" })).toBeInTheDocument();
  expect(screen.getByText("1 foto · 1 quadro vazio")).toBeInTheDocument();
  await waitFor(() => expect(screen.getByRole("slider", { name: "Zoom da foto" })).toBeEnabled());
  expect(screen.queryByRole("button", { name: "Design da lâmina" })).not.toBeInTheDocument();
  expect(canvasHarness.props?.selectedFrameIds).toEqual(["frame-001", "frame-002"]);
  act(() => canvasHarness.props?.onSelectFrame("frame-002", true));
  expect(screen.getByText("Quadro selecionado")).toBeInTheDocument();

  act(() => canvasHarness.props?.onSelectFrame(null));
  expect(
    screen.getByRole("button", { name: "Design da lâmina" }),
  ).toBeInTheDocument();

  fireEvent.keyDown(window, { key: "Escape" });
  expect(
    screen.getByRole("button", { name: "Informações do álbum" }),
  ).toBeInTheDocument();
});
