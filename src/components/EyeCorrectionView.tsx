import { useEffect, useRef, useState } from "react";
import { Check, ChevronLeft, ChevronRight, Columns2, EyeOff, RefreshCcw, Save, Scan } from "lucide-react";
import type { ViewerCorrectionAction, ViewerPresentation } from "../application/imageViewerWindow";
import { detectFaces, faceBounds, type Face } from "../image-viewer/faceLandmarks";
import { ImageToolButton } from "../ui/ImageToolButton";
import { AppIcon } from "../ui/AppIcon";
import { useUiAnchoredTooltip } from "../ui/UiAnchoredTooltip";

interface Props {
  presentation: ViewerPresentation;
  onNavigate(offset: -1 | 1): void;
  onCorrection(action: ViewerCorrectionAction): void;
}

type Side = "reference" | "target";
type AnalysisState = "idle" | "analyzing" | "ready" | "no-face" | "failed";
// Visual QA only: stable marker positions over the local QA photos. This does not exercise detection.
const fixtureFaces = import.meta.env.DEV && new URLSearchParams(location.search).get("fixture") === "faces";
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

function analysisIssue(state: AnalysisState, photo: "foto de destino" | "referência") {
  if (state === "no-face") return `Nenhum rosto encontrado na ${photo}.`;
  if (state === "failed") return `Não foi possível analisar a ${photo}. Escolha outra foto.`;
  return null;
}

function FaceImage({ side, url, name, notice, choosing, selected, onChoose, onStatus, navigation }: {
  side: Side; url: string | null; name: string; choosing: boolean;
  notice?: string | null;
  selected: number | null; onChoose(index: number, faces: Face[]): void;
  onStatus(state: AnalysisState): void;
  navigation?: { previous: boolean; next: boolean; onNavigate(offset: -1 | 1): void };
}) {
  const image = useRef<HTMLImageElement>(null);
  const pane = useRef<HTMLDivElement>(null);
  const photo = useRef<HTMLDivElement>(null);
  const noticeTooltip = useUiAnchoredTooltip(url ? photo : pane, notice ?? "", !notice);
  const generation = useRef(0);
  const [faces, setFaces] = useState<Face[]>([]);
  const [analysis, setAnalysis] = useState<AnalysisState>("idle");
  useEffect(() => { onStatus(analysis); }, [analysis, onStatus]);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [natural, setNatural] = useState({ width: 0, height: 0 });
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const drag = useRef<{ x: number; y: number; panX: number; panY: number; id: number; moved: boolean; captured: boolean; faceButton: HTMLButtonElement | null } | null>(null);
  const suppressFaceClick = useRef<HTMLButtonElement | null>(null);
  useEffect(() => {
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
    const request = ++generation.current;
    setFaces([]);
    setAnalysis(choosing ? "analyzing" : "idle");
    const element = image.current;
    if (!choosing) return;
    if (!url) { setAnalysis("failed"); return; }
    if (!element?.complete || !element.naturalWidth) return;
    setNatural({ width: element.naturalWidth, height: element.naturalHeight });
    if (fixtureFaces) { setFaces([fixtureFace(side)]); setAnalysis("ready"); return; }
    if (fixtureNoFaces) { setAnalysis("no-face"); return; }
    if (fixtureAnalysisError) { setAnalysis("failed"); return; }
    void detectFaces(element).then((found) => {
      if (generation.current !== request) return;
      setFaces(found);
      setAnalysis(found.length ? "ready" : "no-face");
    }).catch(() => {
      if (generation.current === request) setAnalysis("failed");
    });
    return () => { generation.current++; };
  }, [url, choosing]);
  const scale = natural.width && natural.height
    ? Math.min(Math.max(0, size.width - 48) / natural.width, Math.max(0, size.height - 48) / natural.height) : 0;
  const imageWidth = natural.width * scale;
  const imageHeight = natural.height * scale;
  const zoomedWidth = imageWidth * zoom;
  const zoomedHeight = imageHeight * zoom;
  const bound = (next: { x: number; y: number }, value: number) => ({
    x: Math.max(-Math.max(0, (imageWidth * value - size.width) / 2), Math.min(Math.max(0, (imageWidth * value - size.width) / 2), next.x)),
    y: Math.max(-Math.max(0, (imageHeight * value - size.height) / 2), Math.min(Math.max(0, (imageHeight * value - size.height) / 2), next.y)),
  });
  const fit = () => { setZoom(1); setPan({ x: 0, y: 0 }); };
  return <div {...(!url ? noticeTooltip.triggerProps : {})} className="eye-correction__pane-image" ref={pane} tabIndex={0} data-analysis={analysis} aria-busy={analysis === "analyzing"}
    onWheel={(event) => { if (!url || !event.deltaY) return; event.preventDefault(); const value = Math.max(1, Math.min(8, zoom * Math.exp(-event.deltaY * 0.002))); setPan(bound(pan, value)); setZoom(value); }}
    onKeyDown={(event) => { if (event.key === "0") { event.preventDefault(); fit(); } else if (event.key === "+" || event.key === "=") { event.preventDefault(); const value = Math.min(8, zoom * 1.25); setPan(bound(pan, value)); setZoom(value); } else if (event.key === "-") { event.preventDefault(); const value = Math.max(1, zoom / 1.25); setPan(bound(pan, value)); setZoom(value); } }}
    onPointerDown={(event) => {
      suppressFaceClick.current = null;
      const button = (event.target as Element).closest<HTMLButtonElement>("button");
      if (zoom <= 1 || event.button !== 0 || (button && !button.classList.contains("eye-correction__face"))) return;
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
      style={{ left: size.width / 2 - zoomedWidth / 2 + pan.x, top: size.height / 2 - zoomedHeight / 2 + pan.y, width: zoomedWidth, height: zoomedHeight }}>
      <img ref={image} alt={name} src={url} crossOrigin="anonymous" draggable={false} style={fixtureDarkPhotos ? { filter: "brightness(.3)" } : undefined} onLoad={(event) => {
      setNatural({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight });
      if (choosing) {
        if (fixtureFaces) { setFaces([fixtureFace(side)]); setAnalysis("ready"); return; }
        if (fixtureNoFaces) { setFaces([]); setAnalysis("no-face"); return; }
        if (fixtureAnalysisError) { setFaces([]); setAnalysis("failed"); return; }
        const request = ++generation.current;
        setAnalysis("analyzing");
        void detectFaces(event.currentTarget).then((found) => {
          if (generation.current !== request) return;
          setFaces(found);
          setAnalysis(found.length ? "ready" : "no-face");
        }).catch(() => { if (generation.current === request) setAnalysis("failed"); });
      }
    }} />
    {choosing && faces.length > 0 && imageWidth > 0 && imageHeight > 0 &&
      <div className="eye-correction__face-layer">
        {faces.map((face, index) => {
          const bounds = faceBounds(face);
          if (!bounds) return null;
          const left = size.width / 2 + (bounds.left - .5) * zoomedWidth + pan.x;
          const top = size.height / 2 + (bounds.top - .5) * zoomedHeight + pan.y;
          if (left >= size.width || top >= size.height || left + (bounds.right - bounds.left) * zoomedWidth <= 0 || top + (bounds.bottom - bounds.top) * zoomedHeight <= 0) return null;
          return <button key={index} type="button" className="eye-correction__face" data-selected={selected === index}
            style={{ left: `${bounds.left * 100}%`, top: `${bounds.top * 100}%`, width: `${(bounds.right - bounds.left) * 100}%`, height: `${(bounds.bottom - bounds.top) * 100}%` }}
            aria-pressed={selected === index} aria-label={`${side === "reference" ? "Referência" : "Imagem a corrigir"}: rosto ${index + 1}`}
            onKeyDown={() => { suppressFaceClick.current = null; }}
            onClick={(event) => { if (suppressFaceClick.current === event.currentTarget) { suppressFaceClick.current = null; return; } onChoose(index, faces); }}>
            {selected === index && <span className="eye-correction__face-check"><AppIcon icon={Check} size={12} /></span>}
          </button>;
        })}
      </div>}
    </div>}
    {navigation && <>
      <ImageToolButton label="Referência anterior" icon={ChevronLeft} glyph="navigation" className="eye-correction__nav--previous" disabled={!navigation.previous} onClick={() => navigation.onNavigate(-1)} />
      <ImageToolButton label="Próxima referência" icon={ChevronRight} glyph="navigation" className="eye-correction__nav--next" disabled={!navigation.next} onClick={() => navigation.onNavigate(1)} />
    </>}
    {zoom > 1 && <ImageToolButton label={`Ajustar ${side === "reference" ? "referência" : "imagem a corrigir"} à janela`} icon={Scan} className="eye-correction__fit" onClick={fit} />}
    {noticeTooltip.tooltip}
  </div>;
}

export function EyeCorrectionView({ presentation, onNavigate, onCorrection }: Props) {
  const correction = presentation.correction!;
  const [targetIndex, setTargetIndex] = useState<number | null>(null);
  const [referenceIndex, setReferenceIndex] = useState<number | null>(null);
  const [targetFaces, setTargetFaces] = useState<Face[]>([]);
  const [referenceFaces, setReferenceFaces] = useState<Face[]>([]);
  const [targetAnalysis, setTargetAnalysis] = useState<AnalysisState>("idle");
  const [referenceAnalysis, setReferenceAnalysis] = useState<AnalysisState>("idle");
  const [selectionRevision, setSelectionRevision] = useState(0);
  const requestedPair = useRef<string | null>(null);
  const [comparison, setComparison] = useState<{ key: string; original: boolean } | null>(null);
  useEffect(() => { setReferenceIndex(null); setReferenceFaces([]); requestedPair.current = null; }, [correction.referenceMediaId, correction.phase === "browse"]);
  const referenceChoosing = correction.phase === "select";
  const targetChoosing = correction.phase === "browse" || correction.phase === "select";
  const compareKey = `${correction.phase}:${correction.referenceMediaId ?? ""}:${correction.resultUrl ?? ""}`;
  const showOriginal = correction.phase === "preview" && comparison?.key === compareKey && comparison.original;
  const busy = correction.phase === "applying";
  const targetIssue = analysisIssue(targetAnalysis, "foto de destino");
  const referenceIssue = analysisIssue(referenceAnalysis, "referência");
  const browseBlocked = !correction.referenceUrl || correction.referenceState !== "ready" || Boolean(targetIssue) || Boolean(correction.error);
  const action = (kind: ViewerCorrectionAction["kind"]) => onCorrection({ sessionId: presentation.sessionId, kind });
  useEffect(() => {
    if (correction.phase !== "select" || targetIndex === null || referenceIndex === null
      || !targetFaces[targetIndex] || !referenceFaces[referenceIndex] || targetIssue || referenceIssue) return;
    const pair = `${presentation.sessionId}:${correction.referenceMediaId}:${selectionRevision}`;
    if (requestedPair.current === pair) return;
    requestedPair.current = pair;
    onCorrection({ sessionId: presentation.sessionId, kind: "preview", referenceMediaId: correction.referenceMediaId,
      targetFace: targetFaces[targetIndex], referenceFace: referenceFaces[referenceIndex] });
  }, [correction.phase, correction.referenceMediaId, presentation.sessionId, selectionRevision,
    targetIndex, referenceIndex, targetFaces, referenceFaces, targetIssue, referenceIssue, onCorrection]);
  return <div className="eye-correction" aria-label="Correção de olhos" aria-busy={correction.phase === "processing" || busy}>
    <div className="eye-correction__tools">
      <ImageToolButton label="Fechar correção" icon={EyeOff} tooltipPlacement="bottom" className="eye-correction__close" disabled={busy} onClick={() => action("cancel")} />
      <div className="eye-correction__tool-extension">
        {(correction.phase === "preview" || busy) && <>
          <ImageToolButton label={showOriginal ? "Antes e depois: mostrar correção" : "Antes e depois: mostrar original"} icon={Columns2} tooltipPlacement="bottom" aria-pressed={showOriginal} disabled={busy || !correction.resultUrl}
            onClick={() => setComparison({ key: compareKey, original: !showOriginal })} />
          <ImageToolButton label="Salvar correção" icon={Save} tooltipPlacement="bottom" disabled={busy} blocked={!correction.resultUrl} onClick={() => action("apply")} />
        </>}
      </div>
    </div>
    <div className="eye-correction__panes">
      <div className="eye-correction__pane">
        <FaceImage side="reference" url={correction.referenceUrl} name={correction.referenceName} notice={referenceIssue} choosing={referenceChoosing}
          selected={referenceIndex} onChoose={(index, faces) => { setReferenceIndex(index); setReferenceFaces(faces); setSelectionRevision((value) => value + 1); }} onStatus={setReferenceAnalysis}
          navigation={correction.phase === "browse" ? { previous: correction.canPreviousReference, next: correction.canNextReference, onNavigate } : undefined} />
        {correction.phase === "browse"
          ? <ImageToolButton label="Usar esta foto" icon={Check} className="eye-correction__reference-action" blocked={browseBlocked} onClick={() => action("select")} />
          : <ImageToolButton label="Trocar referência" icon={RefreshCcw} className="eye-correction__reference-action" disabled={busy} onClick={() => action("browse")} />}
      </div>
      <div className="eye-correction__pane">
        <FaceImage side="target" url={(correction.phase === "preview" || correction.phase === "applying") && correction.resultUrl && !showOriginal ? correction.resultUrl : presentation.url}
          name={presentation.name} notice={correction.error ?? targetIssue} choosing={targetChoosing} selected={targetIndex}
          onChoose={(index, faces) => { setTargetIndex(index); setTargetFaces(faces); setSelectionRevision((value) => value + 1); }} onStatus={setTargetAnalysis} />
      </div>
    </div>
  </div>;
}
