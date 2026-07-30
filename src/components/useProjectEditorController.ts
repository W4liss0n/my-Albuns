import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type {
  ExportPort,
  ExportResult,
  ProjectSessionPort,
} from "../application/projectPorts";
import type {
  EditorProjection,
  ProjectIntent,
} from "../domain/project";
import { useEditorView } from "../state/editorView";
import type {
  AlbumCanvasProps,
  CanvasMetrics,
  PhotoTransformPreview,
} from "./albumCanvasContract";
import { createContinuousCanvasLayout } from "./canvasGeometry";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

interface ProjectEditorControllerInput {
  projection: EditorProjection;
  exportPort: ExportPort;
  projectSessionPort: ProjectSessionPort;
  onProjectionChange(projection: EditorProjection): void;
}

interface ZoomDraft {
  projectId: string;
  frameId: string;
  startValue: number;
  value: number;
  committing: boolean;
}

interface ScopedPhotoTransformPreview {
  projectId: string;
  preview: PhotoTransformPreview;
}

interface ExportContext {
  exportPort: ExportPort;
  projectId: string;
  current: boolean;
}

function messageFromError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export function useProjectEditorController({
  projection,
  exportPort,
  projectSessionPort,
  onProjectionChange,
}: ProjectEditorControllerInput) {
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
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [exportResult, setExportResult] =
    useState<ExportResult | null>(null);
  const [canvasMetrics, setCanvasMetrics] =
    useState<CanvasMetrics | null>(null);
  const [zoomDraft, setZoomDraftState] = useState<ZoomDraft | null>(
    null,
  );
  const zoomDraftRef = useRef<ZoomDraft | null>(null);
  const pendingSheetNavigationRef = useRef<string | null>(null);
  const runProjectMutation = useProjectMutationRunner(
    projection.state.projectId,
    projectSessionPort,
  );
  const feedbackTokenRef = useRef(0);
  const exportContext = useMemo<ExportContext>(
    () => ({
      exportPort,
      projectId: projection.state.projectId,
      current: false,
    }),
    [exportPort, projection.state.projectId],
  );
  const [canvasPhotoPreview, setCanvasPhotoPreview] =
    useState<ScopedPhotoTransformPreview | null>(null);

  useLayoutEffect(() => {
    exportContext.current = true;
    return () => {
      exportContext.current = false;
    };
  }, [exportContext]);

  useLayoutEffect(() => {
    synchronizeProject(
      projection.state.projectId,
      projection.state.album.sheets.map((sheet) => sheet.id),
      projection.state.album.sheets.flatMap((sheet) =>
        sheet.frames.map((frame) => frame.id),
      ),
    );
  }, [projection.state, synchronizeProject]);

  const selectedFrame = useMemo(
    () =>
      projection.state.album.sheets
        .flatMap((sheet) => sheet.frames)
        .find((frame) => frame.id === selectedFrameId) ?? null,
    [projection.state.album.sheets, selectedFrameId],
  );
  const selectedComposedPhoto = useMemo(
    () =>
      projection.composition.sheets
        .flatMap((sheet) => sheet.frames)
        .find((frame) => frame.frameId === selectedFrameId)?.photo ??
      null,
    [projection.composition.sheets, selectedFrameId],
  );
  const canvasLayout = useMemo(
    () =>
      createContinuousCanvasLayout(projection.composition.sheets),
    [projection.composition.sheets],
  );

  function setZoomDraft(next: ZoomDraft | null) {
    zoomDraftRef.current = next;
    setZoomDraftState(next);
  }

  useEffect(() => {
    pendingSheetNavigationRef.current = null;
    setBusy(null);
    setMessage(null);
    setExportResult(null);
    setZoomDraft(null);
    setCanvasPhotoPreview(null);
  }, [exportPort, projectSessionPort, projection.state.projectId]);

  function centerCanvasOnSheet(
    sheetId: string,
    metrics: CanvasMetrics,
  ) {
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
  }

  function handleCanvasMetricsChange(metrics: CanvasMetrics) {
    setCanvasMetrics(metrics);
    const pendingSheetId = pendingSheetNavigationRef.current;
    if (
      pendingSheetId &&
      centerCanvasOnSheet(pendingSheetId, metrics)
    ) {
      pendingSheetNavigationRef.current = null;
    }
  }

  function navigateToSheet(sheetId: string) {
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
  }

  async function runWithGlobalFeedback(
    label: string,
    operation: (
      port: ProjectSessionPort,
    ) => Promise<EditorProjection>,
  ) {
    const feedbackToken = feedbackTokenRef.current + 1;
    feedbackTokenRef.current = feedbackToken;
    setBusy(label);
    setMessage(null);
    const outcome = await runProjectMutation(operation);
    if (outcome.status === "completed") {
      onProjectionChange(outcome.projection);
    } else if (
      outcome.status === "failed" &&
      feedbackToken === feedbackTokenRef.current
    ) {
      setMessage(messageFromError(outcome.error));
    }
    if (feedbackToken === feedbackTokenRef.current) {
      setBusy(null);
    }
  }

  function applyWithStatus(intent: ProjectIntent) {
    return runWithGlobalFeedback("Aplicando alteração", (port) =>
      port.apply(intent),
    );
  }

  async function commitInteraction(intent: ProjectIntent) {
    setMessage(null);
    const outcome = await runProjectMutation((port) =>
      port.apply(intent),
    );
    if (outcome.status === "completed") {
      onProjectionChange(outcome.projection);
      return true;
    }
    if (outcome.status === "failed") {
      setCanvasPhotoPreview(null);
      setMessage(messageFromError(outcome.error));
    }
    return false;
  }

  async function exportPreview() {
    const context = exportContext;
    const feedbackToken = feedbackTokenRef.current + 1;
    feedbackTokenRef.current = feedbackToken;
    setBusy("Exportando");
    setMessage(null);
    try {
      const result = await context.exportPort.exportPreview();
      if (context.current) {
        setExportResult(result);
      }
    } catch (error: unknown) {
      if (
        context.current &&
        feedbackToken === feedbackTokenRef.current
      ) {
        setMessage(messageFromError(error));
      }
    } finally {
      if (
        context.current &&
        feedbackToken === feedbackTokenRef.current
      ) {
        setBusy(null);
      }
    }
  }

  function beginZoomGesture() {
    if (!selectedFrame?.photo) return;
    const projectId = projection.state.projectId;
    const frameId = selectedFrame.id;
    const currentValue = selectedFrame.photo.transform.userZoom;
    const currentDraft = zoomDraftRef.current;
    if (
      currentDraft?.projectId === projectId &&
      currentDraft.frameId === frameId &&
      !currentDraft.committing
    ) {
      return;
    }
    setZoomDraft({
      projectId,
      frameId,
      startValue: currentValue,
      value: currentValue,
      committing: false,
    });
  }

  function updateZoomGesture(nextValue: number) {
    if (!selectedFrame?.photo) return;
    const projectId = projection.state.projectId;
    const frameId = selectedFrame.id;
    const currentValue = selectedFrame.photo.transform.userZoom;
    const currentDraft = zoomDraftRef.current;
    const draft =
      currentDraft?.projectId === projectId &&
      currentDraft.frameId === frameId &&
      !currentDraft.committing
        ? currentDraft
        : {
            projectId,
            frameId,
            startValue: currentValue,
            value: currentValue,
            committing: false,
          };
    setZoomDraft({
      ...draft,
      value: nextValue,
    });
  }

  async function finishZoomGesture() {
    if (!selectedFrame) return;
    const projectId = projection.state.projectId;
    const frameId = selectedFrame.id;
    const draft = zoomDraftRef.current;
    if (
      !draft ||
      draft.projectId !== projectId ||
      draft.frameId !== frameId ||
      draft.committing
    ) {
      return;
    }

    const delta = Number((draft.value - draft.startValue).toFixed(4));
    if (Math.abs(delta) < 0.0001) {
      setZoomDraft(null);
      return;
    }

    const committingDraft = { ...draft, committing: true };
    setZoomDraft(committingDraft);
    await commitInteraction({
      kind: "transformPhoto",
      frameId,
      deltaPanX: 0,
      deltaPanY: 0,
      deltaZoom: delta,
    });
    if (zoomDraftRef.current === committingDraft) {
      setZoomDraft(null);
    }
  }

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey) return;
      if (
        event.key.toLocaleLowerCase() === "z" &&
        projection.state.canUndo
      ) {
        event.preventDefault();
        void runWithGlobalFeedback("Desfazendo", (port) =>
          port.undo(),
        );
      }
      if (
        event.key.toLocaleLowerCase() === "y" &&
        projection.state.canRedo
      ) {
        event.preventDefault();
        void runWithGlobalFeedback("Refazendo", (port) =>
          port.redo(),
        );
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  });

  useEffect(() => {
    if (
      zoomDraftRef.current &&
      (zoomDraftRef.current.projectId !==
        projection.state.projectId ||
        zoomDraftRef.current.frameId !== selectedFrameId)
    ) {
      setZoomDraft(null);
    }
    setCanvasPhotoPreview((current) =>
      current?.projectId === projection.state.projectId &&
      current.preview.frameId === selectedFrameId
        ? current
        : null,
    );
  }, [projection.state.projectId, selectedFrameId]);

  useEffect(() => {
    setCanvasPhotoPreview(null);
  }, [projection]);

  const selectedPhotoZoom =
    selectedFrame?.photo?.transform.userZoom ?? 1;
  const selectedCanvasPhotoPreview =
    canvasPhotoPreview?.projectId === projection.state.projectId &&
    canvasPhotoPreview.preview.frameId === selectedFrame?.id
      ? canvasPhotoPreview.preview
      : null;
  const displayedPhotoZoom =
    zoomDraft &&
    zoomDraft.projectId === projection.state.projectId &&
    zoomDraft.frameId === selectedFrame?.id
      ? zoomDraft.value
      : (selectedCanvasPhotoPreview?.zoom ?? selectedPhotoZoom);
  const displayedPhotoPanX =
    selectedCanvasPhotoPreview?.panX ??
    selectedFrame?.photo?.transform.panX ??
    0;
  const implicitSheetId = projection.state.album.sheets.some(
    (sheet) => sheet.id === centeredSheetId,
  )
    ? centeredSheetId
    : projection.state.album.sheets[0]?.id;

  const canvasProps: AlbumCanvasProps = {
    projectId: projection.state.projectId,
    composition: projection.composition,
    continuousCanvasLayout: canvasLayout,
    selectedFrameId,
    focusedSheetId,
    centeredSheetId,
    viewport,
    photoZoomPreview:
      zoomDraft?.projectId === projection.state.projectId
        ? {
            frameId: zoomDraft.frameId,
            value: zoomDraft.value,
          }
        : null,
    onSelectFrame: selectFrame,
    onFocusSheet: focusSheet,
    onCenteredSheetChange: centerSheet,
    onViewportChange: setViewport,
    onTransformPreview: (preview) =>
      setCanvasPhotoPreview(
        preview
          ? {
              projectId: projection.state.projectId,
              preview,
            }
          : null,
      ),
    onTransformCommit: (delta) =>
      commitInteraction({
        kind: "transformPhoto",
        ...delta,
      }),
    onCanvasMetricsChange: handleCanvasMetricsChange,
  };

  return {
    busy,
    message,
    exportResult,
    selectedFrame,
    selectedComposedPhoto,
    displayedPhotoZoom,
    displayedPhotoPanX,
    zoomCommitting: Boolean(
      zoomDraft?.projectId === projection.state.projectId &&
        zoomDraft.committing,
    ),
    sheetCount: projection.state.album.sheets.length,
    photoCount: projection.state.album.sheets.reduce(
      (count, sheet) =>
        count + sheet.frames.filter((frame) => frame.photo).length,
      0,
    ),
    canvasProps,
    navigateToSheet,
    beginZoomGesture,
    updateZoomGesture,
    finishZoomGesture,
    undo: () =>
      void runWithGlobalFeedback("Desfazendo", (port) =>
        port.undo(),
      ),
    redo: () =>
      void runWithGlobalFeedback("Refazendo", (port) =>
        port.redo(),
      ),
    exportPreview: () => void exportPreview(),
    fillMedia: (mediaId: string) => {
      if (implicitSheetId) {
        void applyWithStatus({
          kind: "fillLeftmostPlaceholder",
          sheetId: implicitSheetId,
          mediaId,
        });
      }
    },
    dismissFeedback: () => {
      setMessage(null);
      setExportResult(null);
    },
  };
}
