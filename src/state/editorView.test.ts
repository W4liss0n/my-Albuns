// @vitest-environment node
import { beforeEach, expect, test } from "vitest";

import { useEditorView } from "./editorView";

beforeEach(() => {
  useEditorView.setState({
    projectId: null,
    selectedFrameIds: [],
    selectedSheetIds: [],
    sheetSelectionAnchorId: null,
    sheetSelectionSource: "focus",
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
    selectedSheetIds: ["sheet-101"],
    sheetSelectionAnchorId: "sheet-101",
    focusedSheetId: "sheet-101",
    centeredSheetId: "sheet-101",
    viewport: { offsetX: 0 },
  });
});

test("a Grade selection of several Sheets collapses when one Sheet is focused", () => {
  const view = useEditorView.getState();
  view.synchronizeProject("project-001", ["sheet-001", "sheet-002", "sheet-003"], []);
  view.selectSheets({
    selectedSheetIds: ["sheet-001", "sheet-003"],
    focusedSheetId: "sheet-003",
    anchorSheetId: "sheet-001",
  });
  expect(useEditorView.getState()).toMatchObject({
    selectedSheetIds: ["sheet-001", "sheet-003"],
    focusedSheetId: "sheet-003",
    sheetSelectionAnchorId: "sheet-001",
    centeredSheetId: "sheet-001",
  });

  view.focusSheet("sheet-002");
  expect(useEditorView.getState()).toMatchObject({
    selectedSheetIds: ["sheet-002"],
    focusedSheetId: "sheet-002",
    sheetSelectionAnchorId: "sheet-002",
  });

  view.selectSheets({
    selectedSheetIds: ["sheet-001", "sheet-002"],
    focusedSheetId: "sheet-002",
    anchorSheetId: "sheet-001",
  });
  view.enterSheetEdit("sheet-001");
  expect(useEditorView.getState()).toMatchObject({
    selectedSheetIds: ["sheet-001"],
    focusedSheetId: "sheet-001",
  });
});

test("only a selection made in the Grade becomes the target of Delete and Duplicate", () => {
  const view = useEditorView.getState();
  view.synchronizeProject("project-001", ["sheet-001", "sheet-002", "sheet-003"], []);
  expect(useEditorView.getState().sheetSelectionSource).toBe("focus");

  view.selectSheets({
    selectedSheetIds: ["sheet-002", "sheet-003"],
    focusedSheetId: "sheet-003",
    anchorSheetId: "sheet-002",
  });
  expect(useEditorView.getState().sheetSelectionSource).toBe("grid");
  view.synchronizeProject("project-001", ["sheet-001", "sheet-003"], []);
  expect(useEditorView.getState()).toMatchObject({
    selectedSheetIds: ["sheet-003"],
    sheetSelectionSource: "grid",
  });
  // Undo or Redo removed every selected Sheet: the fallback is not a choice.
  view.synchronizeProject("project-001", ["sheet-001"], []);
  expect(useEditorView.getState()).toMatchObject({
    selectedSheetIds: ["sheet-001"],
    sheetSelectionSource: "focus",
  });

  view.synchronizeProject("project-001", ["sheet-001", "sheet-002"], []);
  view.selectSheets({
    selectedSheetIds: ["sheet-002"],
    focusedSheetId: "sheet-002",
    anchorSheetId: "sheet-002",
  });
  view.focusSheet("sheet-001");
  expect(useEditorView.getState().sheetSelectionSource).toBe("focus");
  view.selectSheets({
    selectedSheetIds: ["sheet-002"],
    focusedSheetId: "sheet-002",
    anchorSheetId: "sheet-002",
  });
  view.enterSheetEdit("sheet-002");
  expect(useEditorView.getState().sheetSelectionSource).toBe("focus");
});

test("prunes deleted Sheets from the selection and keeps one focused", () => {
  const view = useEditorView.getState();
  view.synchronizeProject("project-001", ["sheet-001", "sheet-002", "sheet-003"], []);
  view.selectSheets({
    selectedSheetIds: ["sheet-001", "sheet-002", "sheet-003"],
    focusedSheetId: "sheet-003",
    anchorSheetId: "sheet-001",
  });

  view.synchronizeProject("project-001", ["sheet-002", "sheet-003"], []);
  expect(useEditorView.getState()).toMatchObject({
    selectedSheetIds: ["sheet-002", "sheet-003"],
    focusedSheetId: "sheet-003",
    sheetSelectionAnchorId: "sheet-003",
  });

  view.synchronizeProject("project-001", ["sheet-002", "sheet-004"], []);
  expect(useEditorView.getState()).toMatchObject({
    selectedSheetIds: ["sheet-002"],
    focusedSheetId: "sheet-002",
  });

  view.synchronizeProject("project-001", ["sheet-004", "sheet-005"], []);
  expect(useEditorView.getState()).toMatchObject({
    selectedSheetIds: ["sheet-004"],
    focusedSheetId: "sheet-004",
  });

  const selectedBefore = useEditorView.getState().selectedSheetIds;
  view.synchronizeProject("project-001", ["sheet-004", "sheet-005"], []);
  expect(useEditorView.getState().selectedSheetIds).toBe(selectedBefore);
});
