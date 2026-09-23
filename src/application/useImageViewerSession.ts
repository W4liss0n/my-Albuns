import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { EditorProjection } from "../domain/project";
import type { MediaFileInfo, MediaPreview } from "./projectPorts";
import type { ImageViewerWindowPort, ViewerPresentation } from "./imageViewerWindow";
import type { ViewerCorrectionPhase } from "../contracts/generated/ViewerCorrectionPhase";
import type { ProjectMutationRunner } from "../components/useProjectMutationRunner";
import { viewerPreviewState } from "./imageViewerModel";

interface CorrectionSession {
  phase: ViewerCorrectionPhase;
  referenceIds: readonly string[];
  referenceMediaId: string;
  token?: string;
  resultUrl?: string;
  error?: string;
}

interface ViewerSession {
  sessionId: string;
  projectId: string;
  source: "panel" | "sheet";
  mediaId: string;
  mediaIds: readonly string[];
  restoreFocus: HTMLElement | null;
  correction?: CorrectionSession;
}

interface Options {
  projectId: string;
  media: EditorProjection["state"]["album"]["media"];
  previews: Readonly<Record<string, MediaPreview>>;
  previewUrls: Readonly<Record<string, string>>;
  files?: Readonly<Record<string, MediaFileInfo>>;
  port?: ImageViewerWindowPort;
  mutation: ProjectMutationRunner;
  onProjectionChange(projection: EditorProjection): void;
}

function restoreOrigin(session: ViewerSession, shouldRestore: () => boolean) {
  requestAnimationFrame(() => {
    if (!shouldRestore()) return;
    (session.restoreFocus?.isConnected ? session.restoreFocus
      : document.querySelector<HTMLElement>(session.source === "panel" ? "#media-panel" : ".canvas-host canvas"))?.focus({ preventScroll: true });
  });
}

/** Owns the viewer lifecycle; the workspace supplies entry context and preview demand. */
export function useImageViewerSession({ projectId, media, previews, previewUrls, files, port, mutation, onProjectionChange }: Options) {
  const [viewer, setViewer] = useState<ViewerSession | null>(null);
  const current = useRef(viewer);
  current.current = viewer;
  const requestGeneration = useRef(0);
  const cancellation = useRef<Promise<void>>(Promise.resolve());
  const openedSession = useRef<string | null>(null);
  const revision = useRef(0);
  const pending = useRef(false);
  const ending = useRef<string | null>(null);
  const active = viewer?.projectId === projectId ? viewer : null;

  const invalidatePreparation = useCallback((cancel: boolean) => {
    requestGeneration.current++;
    if (cancel) cancellation.current = port?.cancelCorrection?.().catch(() => undefined) ?? Promise.resolve();
  }, [port]);

  useEffect(() => {
    invalidatePreparation(current.current !== null);
    setViewer(null);
    pending.current = false;
  }, [projectId]);

  const open = useCallback((source: "panel" | "sheet", mediaId: string, mediaIds: readonly string[], focus: HTMLElement | null) => {
    ending.current = null;
    pending.current = true;
    setViewer({ sessionId: crypto.randomUUID(), projectId, source, mediaId, mediaIds: [...mediaIds], restoreFocus: focus });
  }, [projectId]);

  const endSession = useCallback((sessionId: string) => {
    const session = current.current;
    if (!session || session.sessionId !== sessionId || ending.current === sessionId) return;
    ending.current = sessionId;
    invalidatePreparation(Boolean(session.correction));
    pending.current = false;
    restoreOrigin(session, () => !pending.current && ending.current === sessionId);
    setViewer((value) => value?.sessionId === sessionId ? null : value);
  }, [invalidatePreparation]);

  const presentation = useMemo<ViewerPresentation | null>(() => {
    if (!active) return null;
    const position = active.mediaIds.indexOf(active.mediaId);
    const item = media.find((candidate) => candidate.id === active.mediaId);
    return {
      sessionId: active.sessionId, revision: ++revision.current,
      mediaId: active.mediaId, name: item?.name ?? "Imagem",
      url: previewUrls[active.mediaId] ?? null,
      state: viewerPreviewState(files?.[active.mediaId], previews[active.mediaId]),
      canPrevious: position > 0, canNext: position >= 0 && position < active.mediaIds.length - 1,
      correction: active.correction ? (() => {
        const referenceId = active.correction!.referenceMediaId;
        const refIndex = active.correction!.referenceIds.indexOf(referenceId);
        const reference = media.find((candidate) => candidate.id === referenceId);
        return {
          phase: active.correction!.phase,
          referenceMediaId: referenceId, referenceName: reference?.name ?? "Imagem",
          referenceUrl: previewUrls[referenceId] ?? null,
          referenceState: viewerPreviewState(files?.[referenceId], previews[referenceId]),
          canPreviousReference: refIndex > 0,
          canNextReference: refIndex < active.correction!.referenceIds.length - 1,
          resultUrl: active.correction!.resultUrl ?? null,
          error: active.correction!.error ?? null,
        };
      })() : undefined,
    };
  }, [active, media, previews, previewUrls, files]);

  useEffect(() => {
    if (!port) return;
    if (!presentation) {
      const session = openedSession.current;
      openedSession.current = null;
      if (session) void port.close(session).catch(() => undefined);
      return;
    }
    if (openedSession.current !== presentation.sessionId) {
      const previousSession = openedSession.current;
      openedSession.current = presentation.sessionId;
      void (async () => {
        if (previousSession) await port.close(previousSession);
        await port.open(presentation);
      })().catch(() => endSession(presentation.sessionId));
    } else {
      void port.update(presentation).catch(() => undefined);
    }
  }, [port, presentation, endSession]);

  useEffect(() => {
    if (!port) return;
    let listening = true;
    let stopNavigate: (() => void) | undefined;
    let stopClosed: (() => void) | undefined;
    void port.onNavigate(({ sessionId, offset }) => {
      if (!listening || (offset !== -1 && offset !== 1)) return;
      const value = current.current;
      if (!value || value.sessionId !== sessionId || value.correction?.phase === "applying") return;
      if (value.correction) invalidatePreparation(true);
      setViewer((session) => {
        if (!session || session.sessionId !== sessionId) return session;
        if (session.correction) {
          const index = session.correction.referenceIds.indexOf(session.correction.referenceMediaId);
          const referenceMediaId = session.correction.referenceIds[index + offset];
          return referenceMediaId ? { ...session, correction: { phase: "browse", referenceIds: session.correction.referenceIds, referenceMediaId } } : session;
        }
        const index = session.mediaIds.indexOf(session.mediaId);
        const mediaId = session.mediaIds[index + offset];
        return mediaId ? { ...session, mediaId } : session;
      });
    }).then((stop) => { if (listening) stopNavigate = stop; else stop(); });
    void port.onClosed((sessionId) => {
      if (!listening) return;
      endSession(sessionId);
    }).then((stop) => { if (listening) stopClosed = stop; else stop(); });
    return () => { listening = false; stopNavigate?.(); stopClosed?.(); };
  }, [port, invalidatePreparation, endSession]);

  useEffect(() => {
    if (!port?.onCorrection) return;
    let listening = true;
    let stop: (() => void) | undefined;
    void port.onCorrection((action) => {
      if (!listening) return;
      const session = current.current;
      if (!session || session.sessionId !== action.sessionId || session.correction?.phase === "applying") return;
      if (action.kind === "start") {
        invalidatePreparation(false);
        const referenceIds = media.filter((item) => item.kind === "photo" && item.id !== session.mediaId).map((item) => item.id);
        setViewer((value) => value && value.sessionId === action.sessionId
          ? { ...value, correction: { phase: "browse", referenceIds, referenceMediaId: referenceIds[0] ?? "", error: referenceIds.length ? undefined : "Adicione outra foto ao projeto para usar como referência." } }
          : value);
        return;
      }
      const correction = session.correction;
      if (!correction) return;
      if (action.kind === "cancel") {
        invalidatePreparation(true);
        setViewer((value) => value && value.sessionId === action.sessionId ? { ...value, correction: undefined } : value);
      } else if (action.kind === "browse") {
        invalidatePreparation(true);
        setViewer((value) => value && value.sessionId === action.sessionId && value.correction
          ? { ...value, correction: { phase: "browse", referenceIds: value.correction.referenceIds, referenceMediaId: value.correction.referenceMediaId } } : value);
      } else if (action.kind === "select") {
        invalidatePreparation(false);
        setViewer((value) => value && value.sessionId === action.sessionId && value.correction
          ? { ...value, correction: { ...value.correction, phase: "select", error: undefined } } : value);
      } else if (action.kind === "preview" && action.targetFace && action.referenceFace
        && action.referenceMediaId === correction.referenceMediaId && port.prepareCorrection) {
        const request = ++requestGeneration.current;
        setViewer((value) => value && value.sessionId === action.sessionId && value.correction
          ? { ...value, correction: { ...value.correction, phase: "processing", error: undefined } } : value);
        void cancellation.current.then(() => {
          if (requestGeneration.current !== request) return null;
          return port.prepareCorrection!({ sessionId: action.sessionId, targetMediaId: session.mediaId,
            referenceMediaId: correction.referenceMediaId, targetFace: action.targetFace!, referenceFace: action.referenceFace! });
        }).then((prepared) => {
          if (!prepared || requestGeneration.current !== request) return;
          setViewer((value) => value && value.sessionId === action.sessionId && value.mediaId === session.mediaId
            && value.correction?.referenceMediaId === correction.referenceMediaId && value.correction.phase === "processing"
            ? { ...value, correction: { ...value.correction, phase: "preview", token: prepared.token, resultUrl: prepared.url } } : value);
        }).catch((error) => {
          if (requestGeneration.current !== request) return;
          setViewer((value) => value && value.sessionId === action.sessionId && value.mediaId === session.mediaId
            && value.correction?.referenceMediaId === correction.referenceMediaId && value.correction.phase === "processing"
            ? { ...value, correction: { ...value.correction, phase: "select", error: error instanceof Error ? error.message : "Não foi possível corrigir os olhos." } } : value);
        });
      } else if (action.kind === "apply" && correction.phase === "preview" && correction.token && port.applyCorrection) {
        const token = correction.token;
        setViewer((value) => value && value.sessionId === action.sessionId && value.correction
          ? { ...value, correction: { ...value.correction, phase: "applying", error: undefined } } : value);
        void mutation.run(() => port.applyCorrection!(action.sessionId, token)).then((outcome) => {
          if (outcome.status === "completed") {
            onProjectionChange(outcome.projection);
            setViewer((value) => value && value.sessionId === action.sessionId ? { ...value, correction: undefined } : value);
          } else if (outcome.status === "failed") {
            setViewer((value) => value && value.sessionId === action.sessionId && value.correction
              ? { ...value, correction: { ...value.correction, phase: "preview", error: outcome.error instanceof Error ? outcome.error.message : "Não foi possível salvar a correção." } } : value);
          }
        });
      }
    }).then((value) => { if (listening) stop = value; else value(); });
    return () => { listening = false; stop?.(); };
  }, [port, media, mutation, onProjectionChange, invalidatePreparation]);

  return { viewer, active, pending, open };
}
