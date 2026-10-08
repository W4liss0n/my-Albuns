import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type { EditorProjection } from "../../domain/project";
import {
  effectiveSelectedSheetIds,
  nextSheetSelection,
  type SheetSelectionModifiers,
} from "../../application/sheetSelection";
import { useEditorView } from "../../state/editorView";
import type { CanvasMetrics } from "../canvas/albumCanvasContract";
import { createNormalCanvasLayout } from "../canvas/canvasSheetViewGeometry";

export function useProjectNavigation(projection: EditorProjection) {
  const selectedFrameIds = useEditorView(
    (state) => state.selectedFrameIds,
  );
  const selectedFrameId = selectedFrameIds.length === 1 ? selectedFrameIds[0] : null;
  const focusedSheetId = useEditorView(
    (state) => state.focusedSheetId,
  );
  const storedSelectedSheetIds = useEditorView(
    (state) => state.selectedSheetIds,
  );
  const sheetSelectionSource = useEditorView(
    (state) => state.sheetSelectionSource,
  );
  const selectedSheetIds = useMemo(
    () => effectiveSelectedSheetIds(storedSelectedSheetIds, focusedSheetId),
    [focusedSheetId, storedSelectedSheetIds],
  );
  const centeredSheetId = useEditorView(
    (state) => state.centeredSheetId,
  );
  const editingSheetId = useEditorView((state) => state.editingSheetId);
  const viewport = useEditorView((state) => state.viewport);
  const selectFrame = useEditorView((state) => state.selectFrame);
  const focusSheet = useEditorView((state) => state.focusSheet);
  const selectSheets = useEditorView((state) => state.selectSheets);
  const centerSheetInView = useEditorView((state) => state.centerSheet);
  const keepSelectedFrames = useEditorView((state) => state.keepSelectedFrames);
  const inspectorSubject = useEditorView((state) => state.inspectorSubject);
  const showCanvasInInspector = useEditorView((state) => state.showCanvasInInspector);
  const reportMediaSelection = useEditorView((state) => state.reportMediaSelection);
  const setViewport = useEditorView((state) => state.setViewport);
  const enterSheetEdit = useEditorView((state) => state.enterSheetEdit);
  const exitSheetEdit = useEditorView((state) => state.exitSheetEdit);
  const synchronizeProject = useEditorView(
    (state) => state.synchronizeProject,
  );
  const [canvasMetrics, setCanvasMetrics] =
    useState<CanvasMetrics | null>(null);
  // Arrows, scrolling and the new Sheet after Add or Duplicate all recenter
  // here. A Frame left on another Sheet would stay selected off screen, where
  // Delete, R, H and V could still change it. Recentering is not a selection
  // gesture, so it leaves the contextual panel's subject alone.
  const centerSheet = useCallback((sheetId: string) => {
    keepSelectedFrames(projection.state.album.sheets
      .find((sheet) => sheet.id === sheetId)?.frames.map((frame) => frame.id) ?? []);
    centerSheetInView(sheetId);
  }, [centerSheetInView, keepSelectedFrames, projection.state.album.sheets]);
  const pendingSheetNavigationRef = useRef<string | null>(null);

  const synchronizeProjection = useCallback((current: EditorProjection) => {
    const editedSheetId = useEditorView.getState().editingSheetId;
    synchronizeProject(
      current.state.projectId,
      current.state.album.sheets.map((sheet) => sheet.id),
      current.state.album.sheets.filter((sheet) =>
        editedSheetId === null || sheet.id === editedSheetId).flatMap((sheet) =>
        sheet.frames.map((frame) => frame.id),
      ),
    );
  }, [synchronizeProject]);
  useLayoutEffect(() => {
    synchronizeProjection(projection);
  }, [editingSheetId, projection, synchronizeProjection]);

  useEffect(() => {
    pendingSheetNavigationRef.current = null;
  }, [projection.state.projectId]);

  const canvasLayout = useMemo(
    () =>
      createNormalCanvasLayout(
        projection.composition.sheets,
        projection.state.document.bleedUm,
      ),
    [
      projection.composition.sheets,
      projection.state.document.bleedUm,
    ],
  );

  const centerCanvasOnSheet = useCallback(
    (sheetId: string, metrics: CanvasMetrics) => {
      const offsetX = canvasLayout.centeredOffset(
        sheetId,
        metrics.scale,
        metrics.width,
      );
      if (offsetX === null) return false;

      setViewport({
        ...useEditorView.getState().viewport,
        offsetX,
      });
      return true;
    },
    [canvasLayout, setViewport],
  );

  const handleCanvasMetricsChange = useCallback(
    (metrics: CanvasMetrics) => {
      setCanvasMetrics(metrics);
      const pendingSheetId = pendingSheetNavigationRef.current;
      if (
        pendingSheetId &&
        centerCanvasOnSheet(pendingSheetId, metrics)
      ) {
        pendingSheetNavigationRef.current = null;
        centerSheet(pendingSheetId);
      }
    },
    [centerCanvasOnSheet, centerSheet],
  );

  const navigateToSheet = useCallback(
    (sheetId: string) => {
      const sheetExists = projection.composition.sheets.some(
        (sheet) => sheet.sheetId === sheetId,
      );
      if (!sheetExists) return;

      focusSheet(sheetId);
      centerSheet(sheetId);
      if (!canvasMetrics) {
        pendingSheetNavigationRef.current = sheetId;
        return;
      }

      pendingSheetNavigationRef.current = null;
      centerCanvasOnSheet(sheetId, canvasMetrics);
    },
    [
      canvasMetrics,
      centerCanvasOnSheet,
      centerSheet,
      focusSheet,
      projection.composition.sheets,
    ],
  );

  // The Grade selects without moving the Canvas; only a double click or Enter
  // navigates. Shift ranges follow the confirmed order, never a drag preview.
  const selectSheet = useCallback(
    (sheetId: string, modifiers: SheetSelectionModifiers) => {
      const state = useEditorView.getState();
      selectSheets(nextSheetSelection(
        {
          selectedSheetIds: state.selectedSheetIds,
          focusedSheetId: state.focusedSheetId,
          anchorSheetId: state.sheetSelectionAnchorId,
        },
        sheetId,
        modifiers,
        projection.state.album.sheets.map((sheet) => sheet.id),
      ));
    },
    [projection.state.album.sheets, selectSheets],
  );

  const navigateToAdjacentSheet = useCallback(
    (direction: "previous" | "next") => {
      const sheetIds = projection.state.album.sheets.map((sheet) => sheet.id);
      const currentCenteredSheetId = useEditorView.getState().centeredSheetId;
      const currentIndex = currentCenteredSheetId
        ? sheetIds.indexOf(currentCenteredSheetId)
        : 0;
      const targetIndex =
        (currentIndex >= 0 ? currentIndex : 0) +
        (direction === "previous" ? -1 : 1);
      const targetSheetId = sheetIds[targetIndex];
      if (targetSheetId) navigateToSheet(targetSheetId);
    },
    [navigateToSheet, projection.state.album.sheets],
  );

  function beginSheetEdit(sheetId: string) {
    const selectedBelongsToSheet = projection.state.album.sheets
      .find((sheet) => sheet.id === sheetId)
      ?.frames.some((frame) => frame.id === selectedFrameId) ?? false;
    enterSheetEdit(sheetId, selectedBelongsToSheet);
  }

  const implicitSheetId = editingSheetId ?? (projection.state.album.sheets.some(
    (sheet) => sheet.id === centeredSheetId,
  )
    ? centeredSheetId
    : projection.state.album.sheets[0]?.id);

  return {
    synchronizeProjection,
    canvasScale: canvasMetrics?.scale ?? null,
    selectedFrameIds,
    selectedFrameId,
    focusedSheetId,
    selectedSheetIds,
    sheetSelectionSource,
    centeredSheetId,
    editingSheetId,
    viewport,
    canvasLayout,
    implicitSheetId,
    inspectorSubject,
    showCanvasInInspector,
    reportMediaSelection,
    selectFrame,
    focusSheet,
    selectSheet,
    centerSheet,
    setViewport,
    enterSheetEdit: beginSheetEdit,
    exitSheetEdit,
    handleCanvasMetricsChange,
    navigateToAdjacentSheet,
    navigateToSheet,
  };
}
