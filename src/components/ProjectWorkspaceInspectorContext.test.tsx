import { act, fireEvent, render, screen } from "@testing-library/react";
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
  AlbumInformationForm: () => <div>Conteúdo de Informações do Álbum</div>,
}));

vi.mock("./AlbumDesignForm", () => ({
  AlbumDesignForm: () => <div>Conteúdo de Design do Álbum</div>,
}));

const exportPipelinePort: ExportPipelinePort = {
  startSheet: () => ({
    cancel: async () => "not_found",
    completion: Promise.resolve({
      status: "completed",
      result: { widthPx: 1, heightPx: 1 },
    }),
  }),
};

const projectCorePort: ProjectCorePort = {
  load: async () => representativeProjection,
  validateAlbumInformation: async () => ({
    errors: [],
    impact: { heightPx: 3_543, pageWidthPx: 3_543, sheetWidthPx: 7_087 },
  }),
  apply: async () => representativeProjection,
  applyWithOutcome: async () => ({
    projection: representativeProjection,
    affectedFrameId: null,
  }),
  importPhoto: async () => ({
    kind: "cancelled",
    projection: representativeProjection,
  }),
  resolvePhotoDropTarget: async () => ({ kind: "invalid" }),
  relink: async () => representativeProjection,
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

test("derives Album, Sheet and Frame Inspector contexts from the editing state", () => {
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projectDialogPort={projectDialogPort}
      projectCorePort={projectCorePort}
      projectWindowPort={projectWindowPort}
      projection={representativeProjection}
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
    screen.getByRole("button", { name: "Informações do Álbum" }),
  ).toBeInTheDocument();

  act(() => canvasHarness.props?.onEditSheet?.("sheet-001"));
  expect(
    screen.getByRole("button", { name: "Design da Lâmina" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Informações do Álbum" }),
  ).not.toBeInTheDocument();

  act(() => canvasHarness.props?.onSelectFrame("frame-001"));
  expect(screen.getByText("Frame selecionado")).toBeInTheDocument();

  act(() => canvasHarness.props?.onSelectFrame(null));
  expect(
    screen.getByRole("button", { name: "Design da Lâmina" }),
  ).toBeInTheDocument();

  fireEvent.keyDown(window, { key: "Escape" });
  expect(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  ).toBeInTheDocument();
});
