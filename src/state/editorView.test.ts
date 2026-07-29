import { beforeEach, expect, test } from "vitest";

import { useEditorView } from "./editorView";

beforeEach(() => {
  useEditorView.setState({
    projectId: null,
    selectedFrameId: null,
    focusedSheetId: null,
    centeredSheetId: null,
    viewport: { offsetX: 0, zoom: 1 },
  });
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
    viewport: { offsetX: 0, zoom: 1 },
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
    viewport: { offsetX: -320, zoom: 1 },
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
    viewport: { offsetX: -320, zoom: 1 },
  });
});

test("resets transient state when another Project is opened", () => {
  useEditorView.setState({
    projectId: "project-001",
    selectedFrameId: "frame-001",
    focusedSheetId: "sheet-002",
    centeredSheetId: "sheet-002",
    viewport: { offsetX: -320, zoom: 1 },
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
    viewport: { offsetX: 0, zoom: 1 },
  });
});
