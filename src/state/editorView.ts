import { create } from "zustand";

import type { SheetSelection } from "../application/sheetSelection";
import type { ViewportState } from "./viewport";

export type { ViewportState } from "./viewport";

interface EditorViewState {
  projectId: string | null;
  selectedFrameIds: readonly string[];
  /** Includes `focusedSheetId`; more than one only after Ctrl or Shift in the Grade. */
  selectedSheetIds: readonly string[];
  sheetSelectionAnchorId: string | null;
  /**
   * "grid" while the selection was made in the Grade; Delete and Duplicate
   * then act on it. Any other way of selecting a Sheet returns to "focus".
   */
  sheetSelectionSource: "grid" | "focus";
  focusedSheetId: string | null;
  centeredSheetId: string | null;
  editingSheetId: string | null;
  viewport: ViewportState;
  selectFrame(frameId: string | null, toggle?: boolean): void;
  selectFrames(frameIds: readonly string[]): void;
  /** Selects one Sheet; any wider Sheet selection collapses to it. */
  focusSheet(sheetId: string): void;
  selectSheets(selection: SheetSelection): void;
  centerSheet(sheetId: string): void;
  enterSheetEdit(sheetId: string, preserveSelectedFrame?: boolean): void;
  exitSheetEdit(): void;
  setViewport(viewport: ViewportState): void;
  synchronizeProject(
    projectId: string,
    sheetIds: readonly string[],
    frameIds: readonly string[],
  ): void;
}

export const useEditorView = create<EditorViewState>((set) => ({
  projectId: null,
  selectedFrameIds: [],
  selectedSheetIds: [],
  sheetSelectionAnchorId: null,
  sheetSelectionSource: "focus",
  focusedSheetId: null,
  centeredSheetId: null,
  editingSheetId: null,
  viewport: {
    offsetX: 0,
  },
  selectFrame: (frameId, toggle = false) => set((state) => ({
    selectedFrameIds: frameId === null ? [] : toggle && state.editingSheetId !== null
      ? state.selectedFrameIds.includes(frameId)
        ? state.selectedFrameIds.filter((id) => id !== frameId)
        : [...state.selectedFrameIds, frameId]
      : [frameId],
  })),
  selectFrames: (frameIds) => set({ selectedFrameIds: [...frameIds] }),
  focusSheet: (focusedSheetId) => set((state) => ({
    focusedSheetId,
    selectedSheetIds: singleSheetSelection(state.selectedSheetIds, focusedSheetId),
    sheetSelectionAnchorId: focusedSheetId,
    sheetSelectionSource: "focus",
  })),
  selectSheets: (selection) => set({
    selectedSheetIds: selection.selectedSheetIds,
    focusedSheetId: selection.focusedSheetId,
    sheetSelectionAnchorId: selection.anchorSheetId,
    sheetSelectionSource: "grid",
  }),
  centerSheet: (centeredSheetId) => set({ centeredSheetId }),
  enterSheetEdit: (editingSheetId, preserveSelectedFrame = false) =>
    set((state) => ({
      editingSheetId,
      focusedSheetId: editingSheetId,
      selectedSheetIds: singleSheetSelection(state.selectedSheetIds, editingSheetId),
      sheetSelectionAnchorId: editingSheetId,
      sheetSelectionSource: "focus",
      centeredSheetId: editingSheetId,
      selectedFrameIds: preserveSelectedFrame ? state.selectedFrameIds : [],
    })),
  exitSheetEdit: () =>
    set({ editingSheetId: null, selectedFrameIds: [] }),
  setViewport: (viewport) => set({ viewport }),
  synchronizeProject: (projectId, sheetIds, frameIds) =>
    set((state) => {
      const firstSheetId = sheetIds[0] ?? null;
      if (state.projectId !== projectId) {
        return {
          projectId,
          selectedFrameIds: [],
          selectedSheetIds: firstSheetId === null ? [] : [firstSheetId],
          sheetSelectionAnchorId: firstSheetId,
          sheetSelectionSource: "focus",
          focusedSheetId: firstSheetId,
          centeredSheetId: firstSheetId,
          editingSheetId: null,
          viewport: { offsetX: 0 },
        };
      }

      const selectedFrameIds = state.selectedFrameIds.filter((id) => frameIds.includes(id));
      const keptSheetIds = state.selectedSheetIds.filter((id) => sheetIds.includes(id));
      const focusedSheetId =
        state.focusedSheetId && sheetIds.includes(state.focusedSheetId)
          ? state.focusedSheetId
          : keptSheetIds[0] ?? firstSheetId;
      const selectedSheetIds =
        keptSheetIds.length === state.selectedSheetIds.length
          ? state.selectedSheetIds
          : keptSheetIds.length > 0 || focusedSheetId === null
            ? keptSheetIds
            : [focusedSheetId];
      return {
        selectedFrameIds: selectedFrameIds.length === state.selectedFrameIds.length
          ? state.selectedFrameIds : selectedFrameIds,
        selectedSheetIds,
        // A Grade selection that lost every Sheet (Undo, Redo) is no longer
        // the user's choice: commands go back to the centered Sheet.
        sheetSelectionSource:
          keptSheetIds.length > 0 ? state.sheetSelectionSource : "focus",
        sheetSelectionAnchorId:
          state.sheetSelectionAnchorId &&
          sheetIds.includes(state.sheetSelectionAnchorId)
            ? state.sheetSelectionAnchorId
            : focusedSheetId,
        focusedSheetId,
        centeredSheetId:
          state.centeredSheetId &&
          sheetIds.includes(state.centeredSheetId)
            ? state.centeredSheetId
            : firstSheetId,
        editingSheetId:
          state.editingSheetId && sheetIds.includes(state.editingSheetId)
            ? state.editingSheetId
            : null,
      };
    }),
}));

function singleSheetSelection(
  current: readonly string[],
  sheetId: string,
): readonly string[] {
  return current.length === 1 && current[0] === sheetId ? current : [sheetId];
}
