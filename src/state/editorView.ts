import { create } from "zustand";

import type { ViewportState } from "./viewport";

export type { ViewportState } from "./viewport";

interface EditorViewState {
  projectId: string | null;
  selectedFrameIds: readonly string[];
  focusedSheetId: string | null;
  centeredSheetId: string | null;
  editingSheetId: string | null;
  viewport: ViewportState;
  selectFrame(frameId: string | null, toggle?: boolean): void;
  selectFrames(frameIds: readonly string[]): void;
  focusSheet(sheetId: string): void;
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
  focusSheet: (focusedSheetId) => set({ focusedSheetId }),
  centerSheet: (centeredSheetId) => set({ centeredSheetId }),
  enterSheetEdit: (editingSheetId, preserveSelectedFrame = false) =>
    set((state) => ({
      editingSheetId,
      focusedSheetId: editingSheetId,
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
          focusedSheetId: firstSheetId,
          centeredSheetId: firstSheetId,
          editingSheetId: null,
          viewport: { offsetX: 0 },
        };
      }

      const selectedFrameIds = state.selectedFrameIds.filter((id) => frameIds.includes(id));
      return {
        selectedFrameIds: selectedFrameIds.length === state.selectedFrameIds.length
          ? state.selectedFrameIds : selectedFrameIds,
        focusedSheetId:
          state.focusedSheetId &&
          sheetIds.includes(state.focusedSheetId)
            ? state.focusedSheetId
            : firstSheetId,
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
