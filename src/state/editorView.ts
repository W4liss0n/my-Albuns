import { create } from "zustand";

export interface ViewportState {
  offsetX: number;
  zoom: number;
}

interface EditorViewState {
  selectedFrameId: string | null;
  focusedSheetId: string;
  centeredSheetId: string;
  viewport: ViewportState;
  selectFrame(frameId: string | null): void;
  focusSheet(sheetId: string): void;
  centerSheet(sheetId: string): void;
  setViewport(viewport: ViewportState): void;
}

export const useEditorView = create<EditorViewState>((set) => ({
  selectedFrameId: null,
  focusedSheetId: "lamina-02",
  centeredSheetId: "lamina-02",
  viewport: {
    offsetX: 42,
    zoom: 1,
  },
  selectFrame: (selectedFrameId) => set({ selectedFrameId }),
  focusSheet: (focusedSheetId) => set({ focusedSheetId }),
  centerSheet: (centeredSheetId) => set({ centeredSheetId }),
  setViewport: (viewport) => set({ viewport }),
}));
