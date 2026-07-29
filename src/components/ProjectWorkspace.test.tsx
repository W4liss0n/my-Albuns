import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import type {
  EditorProjection,
  ProjectBridge,
} from "../domain/project";
import { useEditorView } from "../state/editorView";
import {
  createTwoSheetProjection,
  representativeProjection,
} from "../test/projectFixtures";
import type {
  CanvasMetrics,
  PhotoTransformDelta,
  PhotoTransformPreview,
} from "./AlbumCanvas";
import { sheetOffsetInCanvasPixels } from "./canvasGeometry";
import { ProjectWorkspace } from "./ProjectWorkspace";

const canvasHarness = vi.hoisted(() => ({
  props: null as null | {
    onCanvasMetricsChange?(metrics: CanvasMetrics): void;
    onCenteredSheetChange?(sheetId: string): void;
    onTransformPreview?(
      preview: PhotoTransformPreview | null,
    ): void;
    onTransformCommit(
      delta: PhotoTransformDelta,
    ): Promise<boolean>;
  },
}));

vi.mock("./AlbumCanvas", () => ({
  AlbumCanvas: (props: typeof canvasHarness.props) => {
    canvasHarness.props = props;
    return <div data-testid="album-canvas" />;
  },
}));

const projection = representativeProjection;
const twoSheetProjection = createTwoSheetProjection();

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
    projectId: projection.state.projectId,
    selectedFrameId: null,
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
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

test("renders each Grade item from its own composed sheet", () => {
  render(
    <ProjectWorkspace
      projection={twoSheetProjection}
      bridge={bridgeWithApply(async () => twoSheetProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(
    screen.getByRole("img", { name: "Prévia da Lâmina 01" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("img", { name: "Prévia da Lâmina 02" }),
  ).toBeInTheDocument();
  expect(screen.getAllByRole("img")).toHaveLength(2);
});

test("centers a Grade navigation target in the visible Canvas", () => {
  render(
    <ProjectWorkspace
      projection={twoSheetProjection}
      bridge={bridgeWithApply(async () => twoSheetProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCanvasMetricsChange?.({
      width: 1_000,
      scale: 0.5,
    });
  });
  fireEvent.click(screen.getByText("02").closest("button")!);

  const targetSheet = twoSheetProjection.composition.sheets[1];
  const targetCenter =
    sheetOffsetInCanvasPixels(
      twoSheetProjection.composition.sheets,
      1,
    ) +
    targetSheet.widthUm / 1_000 / 2;
  expect(useEditorView.getState().viewport.offsetX).toBeCloseTo(
    1_000 / 2 - targetCenter * 0.5,
  );
});

test("completes Grade navigation requested before Canvas metrics exist", () => {
  render(
    <ProjectWorkspace
      projection={twoSheetProjection}
      bridge={bridgeWithApply(async () => twoSheetProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(screen.getByText("02").closest("button")!);
  expect(useEditorView.getState().viewport.offsetX).toBe(42);

  act(() => {
    canvasHarness.props?.onCanvasMetricsChange?.({
      width: 1_000,
      scale: 0.5,
    });
  });

  expect(useEditorView.getState().viewport.offsetX).toBeCloseTo(24);
  expect(useEditorView.getState().focusedSheetId).toBe("sheet-002");
  expect(useEditorView.getState().centeredSheetId).toBe("sheet-002");
});

test("resizes both workspace panels with persistent splitters", () => {
  const firstView = render(
    <ProjectWorkspace
      projection={projection}
      bridge={bridgeWithApply(async () => projection)}
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
  fireEvent.pointerUp(window, { pointerId: 1 });

  fireEvent.pointerDown(horizontalSplitter, { pointerId: 2 });
  fireEvent.pointerMove(window, { clientX: 0, clientY: 600 });
  fireEvent.pointerUp(window, { pointerId: 2 });

  expect(workspace.getAttribute("style")).toContain(
    "--inspector-width: 350px",
  );
  expect(workspace.getAttribute("style")).toContain(
    "--media-panel-height: 200px",
  );
  expect(localStorage.getItem("myalbuns.workspace.inspector-width")).toBe(
    "350",
  );
  expect(localStorage.getItem("myalbuns.workspace.media-panel-height")).toBe(
    "200",
  );

  firstView.unmount();
  render(
    <ProjectWorkspace
      projection={projection}
      bridge={bridgeWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );
  expect(
    screen
      .getByRole("separator", {
        name: "Redimensionar Painel contextual",
      })
      .parentElement?.getAttribute("style"),
  ).toContain("--inspector-width: 350px");
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
    kind: "transformPhoto",
    frameId: "frame-001",
    deltaPanX: 0,
    deltaPanY: 0,
    deltaZoom: 0.25,
  });
  expect(screen.queryByText("Aplicando alteração")).not.toBeInTheDocument();
  expect(exportButton).toBeEnabled();

  await act(async () => {
    pending.resolve(projection);
    await pending.promise;
  });
});

test("updates the contextual Zoom slider during a Canvas gesture", () => {
  const apply = vi.fn(async () => projection);
  useEditorView.setState({ selectedFrameId: "frame-001" });

  render(
    <ProjectWorkspace
      projection={projection}
      bridge={bridgeWithApply(apply)}
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
  useEditorView.setState({ selectedFrameId: "frame-001" });

  render(
    <ProjectWorkspace
      projection={projection}
      bridge={bridgeWithApply(apply)}
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
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Falha simulada",
  );
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
  const oldBridge = bridgeWithApply(oldApply);
  const newBridge = bridgeWithApply(newApply);
  useEditorView.setState({ selectedFrameId: "frame-001" });

  const view = render(
    <ProjectWorkspace
      projection={projection}
      bridge={oldBridge}
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
      projection={otherProject}
      bridge={newBridge}
      onProjectionChange={onProjectionChange}
    />,
  );
  act(() => useEditorView.setState({ selectedFrameId: "frame-001" }));
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
  });
});

test("uses the Canvas-centered sheet for a media double click", () => {
  const apply = vi.fn(async () => twoSheetProjection);

  render(
    <ProjectWorkspace
      projection={twoSheetProjection}
      bridge={bridgeWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCenteredSheetChange?.("sheet-002");
  });
  fireEvent.doubleClick(screen.getByText("Campo.jpg").closest("button")!);

  expect(apply).toHaveBeenCalledWith({
    kind: "fillLeftmostPlaceholder",
    sheetId: "sheet-002",
    mediaId: "media-002",
  });
});

test("forwards simultaneous Canvas Pan and Zoom as one intent", () => {
  const apply = vi.fn(async () => projection);

  render(
    <ProjectWorkspace
      projection={projection}
      bridge={bridgeWithApply(apply)}
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
  });
});

test("serializes Project mutations so projections cannot arrive out of order", async () => {
  const first = deferredProjection();
  const second = deferredProjection();
  const apply = vi
    .fn<ProjectBridge["apply"]>()
    .mockImplementationOnce(() => first.promise)
    .mockImplementationOnce(() => second.promise);
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      projection={projection}
      bridge={bridgeWithApply(apply)}
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

test("ignores a pending mutation result after the Workspace changes Project", async () => {
  const pending = deferredProjection();
  const apply = vi.fn(() => pending.promise);
  const onProjectionChange = vi.fn();
  const view = render(
    <ProjectWorkspace
      projection={projection}
      bridge={bridgeWithApply(apply)}
      onProjectionChange={onProjectionChange}
    />,
  );

  canvasHarness.props?.onTransformCommit({
    frameId: "frame-001",
    deltaPanX: 0.1,
    deltaPanY: 0,
    deltaZoom: 0,
  });
  const otherProject = {
    ...projection,
    state: {
      ...projection.state,
      projectId: "project-spike-002",
    },
  };
  view.rerender(
    <ProjectWorkspace
      projection={otherProject}
      bridge={bridgeWithApply(apply)}
      onProjectionChange={onProjectionChange}
    />,
  );

  await act(async () => {
    pending.resolve({
      ...projection,
      state: { ...projection.state, revision: 26 },
    });
    await pending.promise;
  });

  expect(onProjectionChange).not.toHaveBeenCalled();
  expect(useEditorView.getState().projectId).toBe("project-spike-002");
});
