import { create } from "zustand";

export interface ViewportState {
  offsetX: number;
  zoom: number;
}

interface EditorViewState {
  projectId: string | null;
  selectedFrameId: string | null;
  focusedSheetId: string | null;
  centeredSheetId: string | null;
  viewport: ViewportState;
  selectFrame(frameId: string | null): void;
  focusSheet(sheetId: string): void;
  centerSheet(sheetId: string): void;
  setViewport(viewport: ViewportState): void;
  synchronizeProject(
    projectId: string,
    sheetIds: readonly string[],
    frameIds: readonly string[],
  ): void;
}

export const useEditorView = create<EditorViewState>((set) => ({
  projectId: null,
  selectedFrameId: null,
  focusedSheetId: null,
  centeredSheetId: null,
  viewport: {
    offsetX: 0,
    zoom: 1,
  },
  selectFrame: (selectedFrameId) => set({ selectedFrameId }),
  focusSheet: (focusedSheetId) => set({ focusedSheetId }),
  centerSheet: (centeredSheetId) => set({ centeredSheetId }),
  setViewport: (viewport) => set({ viewport }),
  synchronizeProject: (projectId, sheetIds, frameIds) =>
    set((state) => {
      const firstSheetId = sheetIds[0] ?? null;
      if (state.projectId !== projectId) {
        return {
          projectId,
          selectedFrameId: null,
          focusedSheetId: firstSheetId,
          centeredSheetId: firstSheetId,
          viewport: { offsetX: 0, zoom: 1 },
        };
      }

      return {
        selectedFrameId:
          state.selectedFrameId &&
          frameIds.includes(state.selectedFrameId)
            ? state.selectedFrameId
            : null,
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
      };
    }),
}));
