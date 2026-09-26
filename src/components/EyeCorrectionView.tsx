import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { Check, ChevronLeft, ChevronRight, EyeOff, RefreshCcw, Save, Scan } from "lucide-react";
import type { ViewerCorrectionAction, ViewerPresentation } from "../application/imageViewerWindow";
import { acquireFaces, faceBounds, type Face } from "../image-viewer/faceLandmarks";
import { ImageToolButton } from "../ui/ImageToolButton";
import { AppIcon } from "../ui/AppIcon";
import { useUiAnchoredTooltip } from "../ui/UiAnchoredTooltip";
import { boundPhotoPan, clampPhotoZoom, fitPhoto, photoIdentity, usePhotoContinuity } from "./photoSurface";

interface Props {
  presentation: ViewerPresentation;
  onNavigate(offset: -1 | 1): void;
  onCorrection(action: ViewerCorrectionAction): void;
  onTargetSettled?(): void;
}

type Side = "reference" | "target";
type AnalysisState = "idle" | "analyzing" | "ready" | "no-face" | "failed";
// Visual QA only: stable marker positions over the local QA photos. This does not exercise detection.
const fixtureFaces = import.meta.env.DEV && new URLSearchParams(location.search).get("fixture") === "faces";
const fixtureMultipleFaces = import.meta.env.DEV && new URLSearchParams(location.search).get("fixture") === "faces-multi";
const fixtureNoFaces = import.meta.env.DEV && new URLSearchParams(location.search).get("fixture") === "none";
const fixtureAnalysisError = import.meta.env.DEV && new URLSearchParams(location.search).get("fixture") === "analysis-error";
const fixtureDarkPhotos = import.meta.env.DEV && new URLSearchParams(location.search).get("fixture-tone") === "dark";
function fixtureFace(side: Side): Face {
  const [centerX, centerY, radiusX, radiusY] = side === "reference"
    ? [.50075, .37648, .14677, .18253] : [.51571, .26458, .14826, .15113];
  return Array.from({ length: 264 }, (_, index) => {
    const angle = index * 2 * Math.PI / 264;
    return { x: centerX + Math.cos(angle) * radiusX, y: centerY + Math.sin(angle) * radiusY, z: 0 };
  });
}
function multiFixtureFaces(side: Side): Face[] {
  // Bounds measured on the local IMG_6246.JPG (target) and IMG_6252.JPG (reference) copies.
  const boxes = side === "target"
    ? [[.60644, .09781, .69993, .26532], [.46493, .20148, .54961, .33397]]
    : [[.47001, .06825, .51970, .15537], [.35603, .12090, .40494, .21069]];
  return boxes.map(([left, top, right, bottom]) => [
    { x: left, y: top, z: 0 }, { x: right, y: top, z: 0 },
    { x: right, y: bottom, z: 0 }, { x: left, y: bottom, z: 0 },
  ]);
}

function analysisIssue(state: AnalysisState, photo: "foto de destino" | "referência") {
  if (state === "no-face") return `Nenhum rosto encontrado na ${photo}.`;
  if (state === "failed") return `Não foi possível analisar a ${photo}. Escolha outra foto.`;
  return null;
}

function FaceImage({ side, url, displayUrl, name, notice, choosing, interactive, retained, visualHidden, selected, onChoose, onStatus, onVisualSettled }: {
  side: Side; url: string | null; name: string; choosing: boolean;
  displayUrl?: string | null; interactive?: boolean; retained?: boolean; visualHidden?: boolean;
  notice?: string | null;
  selected: number | null; onChoose(index: number, faces: Face[]): void;
  onStatus(state: AnalysisState): void;
  onVisualSettled?(visible: boolean): void;
}) {
  const image = useRef<HTMLImageElement>(null);
  const pane = useRef<HTMLDivElement>(null);
  const photo = useRef<HTMLDivElement>(null);
  const noticeTooltip = useUiAnchoredTooltip(url ? photo : pane, notice ?? "", !notice);
  const [faces, setFaces] = useState<Face[]>([]);
  const [analysis, setAnalysis] = useState<AnalysisState>("idle");
  const [loadedUrl, setLoadedUrl] = useState<string | null>(null);
  useEffect(() => { onStatus(analysis); }, [analysis, onStatus]);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [natural, setNatural] = useState({ width: 0, height: 0 });
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const drag = useRef<{ x: number; y: number; panX: number; panY: number; id: number; moved: boolean; captured: boolean; faceButton: HTMLButtonElement | null } | null>(null);
  const suppressFaceClick = useRef<HTMLButtonElement | null>(null);
  useLayoutEffect(() => {
    const element = pane.current;
    if (!element) return;
    const resize = () => setSize({ width: element.clientWidth, height: element.clientHeight });
    resize();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(resize);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    setFaces([]);
    setAnalysis(choosing ? "analyzing" : "idle");
    const element = image.current;
    if (!choosing) return;
    if (!url) { setAnalysis("failed"); return; }
    if (loadedUrl !== url || !element?.naturalWidth) return;
    setNatural({ width: element.naturalWidth, height: element.naturalHeight });
    if (fixtureFaces || fixtureMultipleFaces) { setFaces(fixtureMultipleFaces ? multiFixtureFaces(side) : [fixtureFace(side)]); setAnalysis("ready"); return; }
    if (fixtureNoFaces) { setAnalysis("no-face"); return; }
    if (fixtureAnalysisError) { setAnalysis("failed"); return; }
    const lease = acquireFaces(element);
    let active = true;
    void lease.promise.then((found) => {
      if (!active) return;
      setFaces(found);
      setAnalysis(found.length ? "ready" : "no-face");
    }).catch(() => {
      if (active) setAnalysis("failed");
    });
    return () => { active = false; lease.release(); };
  }, [url, choosing, loadedUrl, side]);
  // The reference pane keeps its 42 px arrows (4 px from the edge) beside the photo.
  const fitted = fitPhoto(natural, size, side === "reference" ? { x: 4 + 42 + 8, y: 24 } : 24, true);
  const imageWidth = fitted.width;
  const imageHeight = fitted.height;
  const canManipulateImage = !retained && !visualHidden && imageWidth > 0 && imageHeight > 0;
  useLayoutEffect(() => {
    if (!onVisualSettled) return;
    if (!url) onVisualSettled(false);
    else if (size.width > 0 && size.height > 0 && imageWidth > 0 && imageHeight > 0 && image.current?.naturalWidth) onVisualSettled(true);
  }, [onVisualSettled, url, size.width, size.height, imageWidth, imageHeight]);
  const zoomedWidth = imageWidth * zoom;
  const zoomedHeight = imageHeight * zoom;
  const bound = (next: { x: number; y: number }, value: number) => boundPhotoPan(next, value, fitted, size);
  const focusFace = (face: Face, value: number) => {
    const bounds = faceBounds(face);
    if (!bounds || value <= 1) return;
    setPan(bound({
      x: (.5 - (bounds.left + bounds.right) / 2) * imageWidth * value,
      y: (.5 - (bounds.top + bounds.bottom) / 2) * imageHeight * value,
    }, value));
  };
  const changeZoom = (value: number) => {
    if (value <= 1) { fit(); return; }
    value = clampPhotoZoom(value);
    const face = selected === null ? null : faces[selected];
    if (face) focusFace(face, value);
    else setPan(bound(pan, value));
    setZoom(value);
  };
  const fit = () => { setZoom(1); setPan({ x: 0, y: 0 }); };
  return <div {...(!url ? noticeTooltip.triggerProps : {})} className="eye-correction__pane-image" ref={pane} tabIndex={retained ? -1 : 0} data-retained={retained || undefined} data-analysis={analysis} aria-busy={analysis === "analyzing"} inert={retained} aria-hidden={retained}
    onWheel={(event) => { if (!canManipulateImage || !event.deltaY) return; event.preventDefault(); changeZoom(clampPhotoZoom(zoom * Math.exp(-event.deltaY * 0.002))); }}
    onKeyDown={(event) => { if (!canManipulateImage) return; if (event.key === "0") { event.preventDefault(); fit(); } else if (event.key === "+" || event.key === "=") { event.preventDefault(); changeZoom(clampPhotoZoom(zoom * 1.25)); } else if (event.key === "-") { event.preventDefault(); changeZoom(clampPhotoZoom(zoom / 1.25)); } }}
    onPointerDown={(event) => {
      suppressFaceClick.current = null;
      const button = (event.target as Element).closest<HTMLButtonElement>("button");
      if (!canManipulateImage || zoom <= 1 || event.button !== 0 || (button && !button.classList.contains("eye-correction__face"))) return;
      drag.current = { x: event.clientX, y: event.clientY, panX: pan.x, panY: pan.y, id: event.pointerId, moved: false, captured: !button, faceButton: button };
      if (!button) event.currentTarget.setPointerCapture(event.pointerId);
    }}
    onPointerMove={(event) => {
      const current = drag.current;
      if (current?.id !== event.pointerId) return;
      const dx = event.clientX - current.x, dy = event.clientY - current.y;
      if (!current.moved && Math.hypot(dx, dy) < 4) return;
      if (!current.captured) { event.currentTarget.setPointerCapture(event.pointerId); current.captured = true; }
      current.moved = true;
      setPan(bound({ x: current.panX + dx, y: current.panY + dy }, zoom));
    }}
    onPointerUp={(event) => { if (drag.current?.id === event.pointerId) { if (drag.current.moved) suppressFaceClick.current = drag.current.faceButton; if (drag.current.captured) event.currentTarget.releasePointerCapture(event.pointerId); drag.current = null; } }}
    onPointerCancel={() => { drag.current = null; }}>
    {url && <div {...noticeTooltip.triggerProps} ref={photo} className="eye-correction__photo" role={notice ? "group" : undefined}
      aria-label={notice ? `${side === "reference" ? "Referência" : "Imagem a corrigir"}: aviso` : undefined} tabIndex={notice ? 0 : undefined}
      style={{ left: size.width / 2 - zoomedWidth / 2 + pan.x, top: size.height / 2 - zoomedHeight / 2 + pan.y, width: zoomedWidth, height: zoomedHeight, visibility: visualHidden ? "hidden" : "visible" }}>
      <img ref={image} alt={name} aria-hidden={Boolean(displayUrl)} src={url} crossOrigin="anonymous" draggable={false} style={fixtureDarkPhotos ? { filter: "brightness(.3)" } : undefined} onLoad={(event) => {
      const loadedImage = event.currentTarget;
      setNatural({ width: loadedImage.naturalWidth, height: loadedImage.naturalHeight });
      setLoadedUrl(url);
    }} onError={() => { setAnalysis("failed"); onVisualSettled?.(false); }} />
    {displayUrl && <img alt={name} src={displayUrl} draggable={false} style={{ position: "absolute", inset: 0, zIndex: 1 }} />}
    {choosing && faces.length > 0 && imageWidth > 0 && imageHeight > 0 &&
      <div className="eye-correction__face-layer">
        {faces.map((face, index) => {
          const bounds = faceBounds(face);
          if (!bounds) return null;
          const left = size.width / 2 + (bounds.left - .5) * zoomedWidth + pan.x;
          const top = size.height / 2 + (bounds.top - .5) * zoomedHeight + pan.y;
          if (left >= size.width || top >= size.height || left + (bounds.right - bounds.left) * zoomedWidth <= 0 || top + (bounds.bottom - bounds.top) * zoomedHeight <= 0) return null;
          return <button key={index} type="button" className="eye-correction__face" data-selected={selected === index}
            disabled={interactive === false}
            style={{ left: `${bounds.left * 100}%`, top: `${bounds.top * 100}%`, width: `${(bounds.right - bounds.left) * 100}%`, height: `${(bounds.bottom - bounds.top) * 100}%` }}
            aria-pressed={selected === index} aria-label={`${side === "reference" ? "Referência" : "Imagem a corrigir"}: rosto ${index + 1}`}
            onKeyDown={() => { suppressFaceClick.current = null; }}
            onClick={(event) => { if (suppressFaceClick.current === event.currentTarget) { suppressFaceClick.current = null; return; } focusFace(face, zoom); onChoose(index, faces); }}>
            {selected === index && <span className="eye-correction__face-check"><AppIcon icon={Check} size={12} /></span>}
          </button>;
        })}
      </div>}
    </div>}
    {(canManipulateImage || retained) && zoom > 1 && <ImageToolButton label={`Ajustar ${side === "reference" ? "referência" : "imagem a corrigir"} à janela`} icon={Scan} className="eye-correction__fit" blocked={retained} aria-busy={retained || undefined} onClick={fit} />}
    {noticeTooltip.tooltip}
  </div>;
}

export function EyeCorrectionView({ presentation, onNavigate, onCorrection, onTargetSettled }: Props) {
  const correction = presentation.correction!;
  const targetIdentity = photoIdentity(presentation.sessionId, presentation.mediaId, presentation.url);
  const referenceIdentity = photoIdentity(presentation.sessionId, correction.referenceMediaId, correction.referenceUrl);
  const [targetIndex, setTargetIndex] = useState<number | null>(null);
  const [referenceIndex, setReferenceIndex] = useState<number | null>(null);
  const [targetSelectionIdentity, setTargetSelectionIdentity] = useState<string | null>(null);
  const [referenceSelectionIdentity, setReferenceSelectionIdentity] = useState<string | null>(null);
  const [targetFaces, setTargetFaces] = useState<Face[]>([]);
  const [referenceFaces, setReferenceFaces] = useState<Face[]>([]);
  const [targetAnalysis, setTargetAnalysis] = useState<AnalysisState>("idle");
  const [referenceAnalysis, setReferenceAnalysis] = useState<AnalysisState>("idle");
  const [selectionRevision, setSelectionRevision] = useState(0);
  const requestedPair = useRef<string | null>(null);
  const [resultRevision, setResultRevision] = useState<number | null>((correction.phase === "preview" || correction.phase === "applying") && correction.resultUrl ? 0 : null);
  const [resultPair, setResultPair] = useState<string | null>(`${targetIdentity}:${referenceIdentity}:null:null`);
  const latestSelectionRevision = useRef(selectionRevision);
  latestSelectionRevision.current = selectionRevision;
  const referencePhoto = usePhotoContinuity<undefined>({ key: referenceIdentity, sessionId: presentation.sessionId,
    url: correction.referenceUrl, name: correction.referenceName, state: correction.referenceState, allowRetain: correction.phase === "browse" });
  const onReferenceSettled = useCallback((visible: boolean) => {
    if (!visible && correction.referenceState === "loading") return;
    if (visible) referencePhoto.markReady(referenceIdentity, undefined);
    else referencePhoto.markFailed(referenceIdentity);
  }, [referenceIdentity, correction.referenceState, referencePhoto.markReady, referencePhoto.markFailed]);
  const previousTargetIdentity = useRef(targetIdentity);
  useEffect(() => {
    if (previousTargetIdentity.current === targetIdentity) return;
    previousTargetIdentity.current = targetIdentity;
    setTargetIndex(null);
    setTargetSelectionIdentity(null);
    setTargetFaces([]);
    setResultRevision(null);
    setResultPair(null);
    setSelectionRevision((value) => value + 1);
    requestedPair.current = null;
  }, [targetIdentity]);
  const previousReferenceIdentity = useRef(referenceIdentity);
  useEffect(() => {
    if (previousReferenceIdentity.current === referenceIdentity) return;
    previousReferenceIdentity.current = referenceIdentity;
    setReferenceIndex(null);
    setReferenceSelectionIdentity(null);
    setReferenceFaces([]);
    setResultRevision(null);
    setResultPair(null);
    setSelectionRevision((value) => value + 1);
    requestedPair.current = null;
  }, [referenceIdentity]);
  useEffect(() => {
    if (correction.phase === "preview" && correction.resultUrl && requestedPair.current) {
      setResultRevision(latestSelectionRevision.current);
      setResultPair(requestedPair.current);
    }
  }, [correction.resultUrl]);
  const [comparison, setComparison] = useState<{ key: string; original: boolean } | null>(null);
  useEffect(() => { setReferenceIndex(null); setReferenceSelectionIdentity(null); setReferenceFaces([]); requestedPair.current = null; }, [correction.referenceMediaId, correction.phase === "browse"]);
  const referenceChoosing = correction.phase !== "browse";
  const targetChoosing = true;
  const referenceReady = referencePhoto.ready;
  const referencePending = referencePhoto.pending;
  const retainedReference = referencePhoto.retained;
  const currentPair = `${targetIdentity}:${referenceIdentity}:${targetIndex}:${referenceIndex}`;
  const currentResult = Boolean(correction.resultUrl && resultRevision === selectionRevision && resultPair === currentPair
    && ((targetSelectionIdentity === targetIdentity && referenceSelectionIdentity === referenceIdentity)
      || (targetIndex === null && referenceIndex === null && selectionRevision === 0))
    && (correction.phase === "preview" || correction.phase === "applying"));
  const retryPreparation = correction.phase === "select" && Boolean(correction.error);
  const compareKey = `${correction.phase}:${correction.referenceMediaId ?? ""}:${correction.resultUrl ?? ""}`;
  const showOriginal = currentResult && correction.phase === "preview" && comparison?.key === compareKey && comparison.original;
  const busy = correction.phase === "applying";
  const targetIssue = analysisIssue(targetAnalysis, "foto de destino");
  const referenceIssue = analysisIssue(referenceAnalysis, "referência");
  const browseBlocked = !referenceReady || !correction.referenceUrl || correction.referenceState !== "ready" || Boolean(targetIssue) || Boolean(correction.error);
  const action = (kind: ViewerCorrectionAction["kind"]) => onCorrection({ sessionId: presentation.sessionId, kind });
  useEffect(() => {
    if (!(correction.phase === "select" || correction.phase === "processing" || correction.phase === "preview") || targetIndex === null || referenceIndex === null
      || targetSelectionIdentity !== targetIdentity || referenceSelectionIdentity !== referenceIdentity
      || !targetFaces[targetIndex] || !referenceFaces[referenceIndex] || targetIssue || referenceIssue) return;
    const pair = currentPair;
    if (requestedPair.current === pair) return;
    requestedPair.current = pair;
    onCorrection({ sessionId: presentation.sessionId, kind: "preview", referenceMediaId: correction.referenceMediaId,
      targetUrl: presentation.url ?? undefined, referenceUrl: correction.referenceUrl ?? undefined,
      targetFace: targetFaces[targetIndex], referenceFace: referenceFaces[referenceIndex] });
  }, [correction.phase, correction.referenceMediaId, presentation.sessionId, targetIdentity, referenceIdentity, selectionRevision,
    targetIndex, referenceIndex, targetSelectionIdentity, referenceSelectionIdentity, targetFaces, referenceFaces, targetIssue, referenceIssue, onCorrection]);
  return <div className="eye-correction" aria-label="Correção de olhos" aria-busy={correction.phase === "processing" || busy}>
    <div className="eye-correction__tools">
      <ImageToolButton label="Fechar correção" icon={EyeOff} tooltipPlacement="bottom" className="eye-correction__close" disabled={busy} onClick={() => action("cancel")} />
      <div className="eye-correction__tool-extension">
        {currentResult && <ImageToolButton label="Salvar correção" icon={Save} tooltipPlacement="bottom" disabled={busy} blocked={!correction.resultUrl} onClick={() => action("apply")} />}
      </div>
    </div>
    <div className="eye-correction__panes">
      <div className="eye-correction__pane">
        {retainedReference && <FaceImage key={retainedReference.key} side="reference" url={retainedReference.url} name={retainedReference.name} choosing={false} interactive={false}
          retained selected={null} onChoose={() => undefined} onStatus={() => undefined} />}
        <FaceImage key={referenceIdentity} side="reference" url={correction.referenceUrl} name={correction.referenceName} notice={referenceIssue} choosing={referenceChoosing} interactive={!busy}
          visualHidden={Boolean(retainedReference)} onVisualSettled={onReferenceSettled}
          selected={referenceSelectionIdentity === referenceIdentity ? referenceIndex : null} onChoose={(index, faces) => {
            if (index === referenceIndex && referenceSelectionIdentity === referenceIdentity && !retryPreparation) return;
            if (retryPreparation) requestedPair.current = null;
            setReferenceIndex(index); setReferenceSelectionIdentity(referenceIdentity); setReferenceFaces(faces); setSelectionRevision((value) => value + 1);
          }} onStatus={setReferenceAnalysis} />
        {correction.phase === "browse" && <>
          <ImageToolButton label="Referência anterior" icon={ChevronLeft} glyph="navigation" className="eye-correction__nav--previous" blocked={!correction.canPreviousReference} onClick={() => onNavigate(-1)} />
          <ImageToolButton label="Próxima referência" icon={ChevronRight} glyph="navigation" className="eye-correction__nav--next" blocked={!correction.canNextReference} onClick={() => onNavigate(1)} />
        </>}
        {correction.phase === "browse"
          ? <ImageToolButton label="Usar esta foto" icon={Check} labelled className="eye-correction__reference-action" blocked={browseBlocked} aria-busy={referencePending || undefined} onClick={() => action("select")} />
          : <ImageToolButton label="Trocar referência" icon={RefreshCcw} labelled className="eye-correction__reference-action" disabled={busy} onClick={() => action("browse")} />}
      </div>
      <div className="eye-correction__pane">
        <FaceImage key={targetIdentity} side="target" url={presentation.url} displayUrl={currentResult && !showOriginal ? correction.resultUrl : null}
          name={presentation.name} notice={correction.error ?? targetIssue} choosing={targetChoosing} interactive={!busy} selected={targetSelectionIdentity === targetIdentity ? targetIndex : null}
          onChoose={(index, faces) => {
            if (index === targetIndex && targetSelectionIdentity === targetIdentity && !retryPreparation) return;
            if (retryPreparation) requestedPair.current = null;
            setTargetIndex(index); setTargetSelectionIdentity(targetIdentity); setTargetFaces(faces); setSelectionRevision((value) => value + 1);
          }} onStatus={setTargetAnalysis} onVisualSettled={onTargetSettled} />
        {/* One button: both versions stay named and a click swaps them. */}
        {currentResult && <button type="button" className="eye-correction__compare" aria-pressed={showOriginal} disabled={busy || !correction.resultUrl}
          aria-label={showOriginal ? "Antes e depois: mostrar correção" : "Antes e depois: mostrar original"}
          onClick={() => setComparison({ key: compareKey, original: !showOriginal })}>
          <span aria-hidden="true" data-current={showOriginal}>Original</span>
          <span aria-hidden="true" data-current={!showOriginal}>Corrigida</span>
        </button>}
      </div>
    </div>
  </div>;
}
