import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type { EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import type { CanvasMetrics } from "./albumCanvasContract";
import { createNormalCanvasLayout } from "./canvasSheetViewGeometry";

export function useProjectNavigation(projection: EditorProjection) {
  const selectedFrameId = useEditorView(
    (state) => state.selectedFrameId,
  );
  const focusedSheetId = useEditorView(
    (state) => state.focusedSheetId,
  );
  const centeredSheetId = useEditorView(
    (state) => state.centeredSheetId,
  );
  const viewport = useEditorView((state) => state.viewport);
  const selectFrame = useEditorView((state) => state.selectFrame);
  const focusSheet = useEditorView((state) => state.focusSheet);
  const centerSheet = useEditorView((state) => state.centerSheet);
  const setViewport = useEditorView((state) => state.setViewport);
  const synchronizeProject = useEditorView(
    (state) => state.synchronizeProject,
  );
  const [canvasMetrics, setCanvasMetrics] =
    useState<CanvasMetrics | null>(null);
  const pendingSheetNavigationRef = useRef<string | null>(null);

  useLayoutEffect(() => {
    synchronizeProject(
      projection.state.projectId,
      projection.state.album.sheets.map((sheet) => sheet.id),
      projection.state.album.sheets.flatMap((sheet) =>
        sheet.frames.map((frame) => frame.id),
      ),
    );
  }, [projection.state, synchronizeProject]);

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

  const implicitSheetId = projection.state.album.sheets.some(
    (sheet) => sheet.id === centeredSheetId,
  )
    ? centeredSheetId
    : projection.state.album.sheets[0]?.id;

  return {
    selectedFrameId,
    focusedSheetId,
    centeredSheetId,
    viewport,
    canvasLayout,
    implicitSheetId,
    selectFrame,
    focusSheet,
    centerSheet,
    setViewport,
    handleCanvasMetricsChange,
    navigateToSheet,
  };
}
