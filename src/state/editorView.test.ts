import { beforeEach, expect, test } from "vitest";

import { useEditorView } from "./editorView";

beforeEach(() => {
  useEditorView.setState({
    projectId: null,
    selectedFrameId: null,
    focusedSheetId: null,
    centeredSheetId: null,
    editingSheetId: null,
    viewport: { offsetX: 0 },
  });
});

test("keeps only a valid Frame selection while editing and clears it on exit", () => {
  useEditorView.setState({
    selectedFrameId: "frame-001",
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
  });

  useEditorView.getState().enterSheetEdit("sheet-001", true);
  expect(useEditorView.getState()).toMatchObject({
    editingSheetId: "sheet-001",
    selectedFrameId: "frame-001",
  });

  useEditorView.getState().exitSheetEdit();
  expect(useEditorView.getState()).toMatchObject({
    editingSheetId: null,
    selectedFrameId: null,
  });

  useEditorView.setState({ selectedFrameId: "frame-002" });
  useEditorView.getState().enterSheetEdit("sheet-001", false);
  expect(useEditorView.getState().selectedFrameId).toBeNull();
});

test("initializes transient navigation from the opened Project", () => {
  useEditorView.getState().synchronizeProject(
    "project-001",
    ["sheet-001", "sheet-002"],
    ["frame-001"],
  );

  expect(useEditorView.getState()).toMatchObject({
    projectId: "project-001",
    selectedFrameId: null,
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
    selectedFrameId: "frame-001",
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
    selectedFrameId: null,
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    viewport: { offsetX: -320 },
  });
});

test("resets transient state when another Project is opened", () => {
  useEditorView.setState({
    projectId: "project-001",
    selectedFrameId: "frame-001",
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
    selectedFrameId: null,
    focusedSheetId: "sheet-101",
    centeredSheetId: "sheet-101",
    viewport: { offsetX: 0 },
  });
});
