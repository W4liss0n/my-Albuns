import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import placementFixture from "../../tests/fixtures/photo-placement-cases.json";
import type {
  EditorProjection,
  PhotoPlacementPlan,
  ProjectBridge,
} from "../domain/project";
import { useEditorView } from "../state/editorView";
import { ProjectWorkspace } from "./ProjectWorkspace";

const canvasHarness = vi.hoisted(() => ({
  props: null as null | {
    onZoomCommit(frameId: string, delta: number): void;
  },
}));

vi.mock("./AlbumCanvas", () => ({
  AlbumCanvas: (props: typeof canvasHarness.props) => {
    canvasHarness.props = props;
    return <div data-testid="album-canvas" />;
  },
}));

const projection: EditorProjection = {
  state: {
    projectId: "project-spike-001",
    projectName: "Álbum Horizonte",
    revision: 25,
    savedRevision: 0,
    dirty: true,
    canUndo: true,
    canRedo: false,
    album: {
      sheets: [
        {
          id: "sheet-001",
          number: 1,
          role: "initial",
          widthUm: 600_000,
          heightUm: 300_000,
          hasOverlay: false,
          frames: [
            {
              id: "frame-001",
              rect: {
                x: 20_000,
                y: 20_000,
                width: 280_000,
                height: 260_000,
              },
              zIndex: 0,
              photo: {
                mediaId: "media-001",
                name: "Serra ao amanhecer.jpg",
                sourceWidthPx: 6_000,
                sourceHeightPx: 4_000,
                palette: ["#10202b", "#648493", "#dfa75e"],
                transform: {
                  panX: 0,
                  panY: 0,
                  userZoom: 1,
                  quarterTurns: 0,
                  fineRotationDegrees: 0,
                  mirrorX: false,
                },
              },
            },
          ],
        },
      ],
      media: [
        {
          id: "media-001",
          name: "Serra ao amanhecer.jpg",
          palette: ["#10202b", "#648493", "#dfa75e"],
          usageCount: 1,
        },
        {
          id: "media-002",
          name: "Campo.jpg",
          palette: ["#21372f", "#92a277", "#e5d7b9"],
          usageCount: 0,
        },
        {
          id: "media-003",
          name: "Praia.jpg",
          palette: ["#123e52", "#428596", "#e7bd76"],
          usageCount: 0,
        },
      ],
    },
  },
  composition: {
    sheets: [
      {
        sheetId: "sheet-001",
        number: 1,
        widthUm: 600_000,
        heightUm: 300_000,
        hasOverlay: false,
        frames: [
          {
            frameId: "frame-001",
            clipRect: {
              x: 20_000,
              y: 20_000,
              width: 280_000,
              height: 260_000,
            },
            zIndex: 0,
            photo: {
              mediaId: "media-001",
              name: "Serra ao amanhecer.jpg",
              drawRect: {
                x: -50_000,
                y: 20_000,
                width: 400_000,
                height: 260_000,
              },
              placement: placementFixture.cases[0]
                .expectedPlan as PhotoPlacementPlan,
              rotationDegrees: 0,
              mirrorX: false,
              palette: ["#10202b", "#648493", "#dfa75e"],
            },
          },
        ],
      },
    ],
  },
};

function deferredProjection() {
  let resolve!: (value: EditorProjection) => void;
  const promise = new Promise<EditorProjection>((resolver) => {
    resolve = resolver;
  });
  return { promise, resolve };
}

function bridgeWithApply(apply: ProjectBridge["apply"]): ProjectBridge {
  return {
    load: async () => projection,
    apply,
    undo: async () => projection,
    redo: async () => projection,
    exportPreview: async () => ({
      outputPath: "C:\\Temp\\Album-Horizonte_001.png",
      widthPx: 600,
      heightPx: 300,
    }),
  };
}

beforeEach(() => {
  canvasHarness.props = null;
  localStorage.clear();
  useEditorView.setState({
    selectedFrameId: null,
    focusedSheetId: "sheet-001",
    viewport: { offsetX: 42, zoom: 0.78 },
  });
});

test("restores accordion preferences after context changes and remounts", () => {
  const renderWorkspace = () =>
    render(
      <ProjectWorkspace
        projection={projection}
        bridge={bridgeWithApply(async () => projection)}
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

  act(() => useEditorView.setState({ selectedFrameId: "frame-001" }));
  expect(
    screen.getByRole("button", { name: "Design" }),
  ).toBeInTheDocument();

  act(() => useEditorView.setState({ selectedFrameId: null }));
  expect(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  ).toHaveAttribute("aria-expanded", "false");

  firstView.unmount();
  renderWorkspace();
  expect(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  ).toHaveAttribute("aria-expanded", "false");
});

test("uses the documented compact chrome and collapsible contextual sections", () => {
  render(
    <ProjectWorkspace
      projection={projection}
      bridge={bridgeWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(screen.queryByLabelText("MyAlbuns")).not.toBeInTheDocument();
  expect(screen.queryByText("Álbum Horizonte")).not.toBeInTheDocument();
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

test("commits a slider zoom once without flashing a global busy state", async () => {
  const pending = deferredProjection();
  const apply = vi.fn(() => pending.promise);
  useEditorView.setState({ selectedFrameId: "frame-001" });

  render(
    <ProjectWorkspace
      projection={projection}
      bridge={bridgeWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const slider = screen.getByRole("slider", { name: "Zoom da Foto" });
  const exportButton = screen.getByRole("button", { name: "Exportar prova" });

  fireEvent.pointerDown(slider);
  fireEvent.change(slider, { target: { value: "112" } });
  fireEvent.change(slider, { target: { value: "125" } });

  expect(apply).not.toHaveBeenCalled();

  fireEvent.pointerUp(slider);

  expect(apply).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith({
    kind: "zoomPhoto",
    frameId: "frame-001",
    delta: 0.25,
  });
  expect(screen.queryByText("Aplicando alteração")).not.toBeInTheDocument();
  expect(exportButton).toBeEnabled();

  await act(async () => {
    pending.resolve(projection);
    await pending.promise;
  });
});
