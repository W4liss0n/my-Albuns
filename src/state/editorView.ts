import { create } from "zustand";

export interface ViewportState {
  offsetX: number;
  zoom: number;
}

interface EditorViewState {
  selectedFrameId: string | null;
  focusedSheetId: string;
  viewport: ViewportState;
  inspectorTab: "album" | "sheets";
  selectFrame(frameId: string | null): void;
  focusSheet(sheetId: string): void;
  setViewport(viewport: ViewportState): void;
  setInspectorTab(tab: "album" | "sheets"): void;
}

export const useEditorView = create<EditorViewState>((set) => ({
  selectedFrameId: null,
  focusedSheetId: "lamina-02",
  viewport: {
    offsetX: 42,
    zoom: 0.78,
  },
  inspectorTab: "album",
  selectFrame: (selectedFrameId) => set({ selectedFrameId }),
  focusSheet: (focusedSheetId) => set({ focusedSheetId }),
  setViewport: (viewport) => set({ viewport }),
  setInspectorTab: (inspectorTab) => set({ inspectorTab }),
}));
