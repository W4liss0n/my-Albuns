import { beforeEach, expect, test } from "vitest";

import { useEditorView } from "./editorView";

beforeEach(() => {
  useEditorView.setState({
    projectId: null,
    selectedFrameIds: [],
    focusedSheetId: null,
    centeredSheetId: null,
    editingSheetId: null,
    viewport: { offsetX: 0 },
  });
});

test("Ctrl-click toggles an editing selection and history only prunes removed Frames", () => {
  const view = useEditorView.getState();
  view.synchronizeProject("project-001", ["sheet-001"], ["frame-001", "frame-002"]);
  view.enterSheetEdit("sheet-001");
  view.selectFrame("frame-001");
  view.selectFrame("frame-002", true);
  expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-001", "frame-002"]);
  view.selectFrame("frame-001", true);
  expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-002"]);
  view.selectFrame("frame-001", true);
  view.synchronizeProject("project-001", ["sheet-001"], ["frame-001"]);
  expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-001"]);
  view.synchronizeProject("project-001", ["sheet-001"], ["frame-001", "frame-002"]);
  expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-001"]);
  view.selectFrame(null);
  expect(useEditorView.getState().selectedFrameIds).toEqual([]);
});

test("keeps only a valid Frame selection while editing and clears it on exit", () => {
  useEditorView.setState({
    selectedFrameIds: ["frame-001"],
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
  });

  useEditorView.getState().enterSheetEdit("sheet-001", true);
  expect(useEditorView.getState()).toMatchObject({
    editingSheetId: "sheet-001",
    selectedFrameIds: ["frame-001"],
  });

  useEditorView.getState().exitSheetEdit();
  expect(useEditorView.getState()).toMatchObject({
    editingSheetId: null,
    selectedFrameIds: [],
  });

  useEditorView.setState({ selectedFrameIds: ["frame-002"] });
  useEditorView.getState().enterSheetEdit("sheet-001", false);
  expect(useEditorView.getState().selectedFrameIds).toEqual([]);
});

test("initializes transient navigation from the opened Project", () => {
  useEditorView.getState().synchronizeProject(
    "project-001",
    ["sheet-001", "sheet-002"],
    ["frame-001"],
  );

  expect(useEditorView.getState()).toMatchObject({
    projectId: "project-001",
    selectedFrameIds: [],
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    viewport: { offsetX: 0 },
  });
});

test("preserves valid view state in the same Session and prunes stale targets", () => {
  useEditorView.getState().synchronizeProject(
    "project-001",
    ["sheet-001", "sheet-002"],
    ["frame-001"],
  );
  useEditorView.setState({
    selectedFrameIds: ["frame-001"],
    focusedSheetId: "sheet-002",
    centeredSheetId: "sheet-002",
    viewport: { offsetX: -320 },
  });

  useEditorView.getState().synchronizeProject(
    "project-001",
    ["sheet-001"],
    [],
  );

  expect(useEditorView.getState()).toMatchObject({
    projectId: "project-001",
    selectedFrameIds: [],
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    viewport: { offsetX: -320 },
  });
});

test("resets transient state when another Project is opened", () => {
  useEditorView.setState({
    projectId: "project-001",
    selectedFrameIds: ["frame-001"],
    focusedSheetId: "sheet-002",
    centeredSheetId: "sheet-002",
    viewport: { offsetX: -320 },
  });

  useEditorView.getState().synchronizeProject(
    "project-002",
    ["sheet-101"],
    ["frame-101"],
  );

  expect(useEditorView.getState()).toMatchObject({
    projectId: "project-002",
    selectedFrameIds: [],
    focusedSheetId: "sheet-101",
    centeredSheetId: "sheet-101",
    viewport: { offsetX: 0 },
  });
});
