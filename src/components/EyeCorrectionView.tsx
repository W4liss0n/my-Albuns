import { useEffect, useRef, useState } from "react";
import { Check, ChevronLeft, ChevronRight, Columns2, EyeOff, RefreshCcw, Save, Scan, Sparkles } from "lucide-react";
import type { ViewerCorrectionAction, ViewerPresentation } from "../application/imageViewerWindow";
import { detectFaces, faceBounds, type Face } from "../image-viewer/faceLandmarks";
import { ImageToolButton } from "../ui/ImageToolButton";
import { useUiAnchoredTooltip } from "../ui/UiAnchoredTooltip";

interface Props {
  presentation: ViewerPresentation;
  onNavigate(offset: -1 | 1): void;
  onCorrection(action: ViewerCorrectionAction): void;
}

type Side = "reference" | "target";
// Visual QA only: stable marker positions over the local QA photos. This does not exercise detection.
const fixtureFaces = import.meta.env.DEV && new URLSearchParams(location.search).get("fixture") === "faces";
const fixtureNoFaces = import.meta.env.DEV && new URLSearchParams(location.search).get("fixture") === "none";
function fixtureFace(side: Side): Face {
  const [centerX, centerY, radiusX, radiusY] = side === "reference"
    ? [.50075, .37648, .14677, .18253] : [.51571, .26458, .14826, .15113];
  return Array.from({ length: 264 }, (_, index) => {
    const angle = index * 2 * Math.PI / 264;
    return { x: centerX + Math.cos(angle) * radiusX, y: centerY + Math.sin(angle) * radiusY, z: 0 };
  });
}

function FaceImage({ side, url, name, choosing, selected, onChoose, onStatus, navigation }: {
  side: Side; url: string | null; name: string; choosing: boolean;
  selected: number | null; onChoose(index: number, faces: Face[]): void;
  onStatus(message: string | null): void;
  navigation?: { previous: boolean; next: boolean; onNavigate(offset: -1 | 1): void };
}) {
  const image = useRef<HTMLImageElement>(null);
  const pane = useRef<HTMLDivElement>(null);
  const generation = useRef(0);
  const [faces, setFaces] = useState<Face[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => { onStatus(message); }, [message, onStatus]);
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
    setMessage(null);
    const element = image.current;
    if (!choosing || !url || !element?.complete || !element.naturalWidth) return;
    setNatural({ width: element.naturalWidth, height: element.naturalHeight });
    if (fixtureFaces) { setFaces([fixtureFace(side)]); return; }
    if (fixtureNoFaces) { setMessage("Nenhum rosto encontrado nesta foto."); return; }
    setMessage("Analisando rostos…");
    void detectFaces(element).then((found) => {
      if (generation.current !== request) return;
      setFaces(found);
      setMessage(found.length ? null : "Nenhum rosto encontrado nesta foto.");
    }).catch(() => {
      if (generation.current === request) setMessage("Não foi possível analisar esta foto. Tente outra.");
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
  return <div className="eye-correction__pane-image" ref={pane} tabIndex={0}
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
    {url ? <img ref={image} alt={name} src={url} crossOrigin="anonymous" draggable={false} style={{ transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})` }} onLoad={(event) => {
      setNatural({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight });
      if (choosing) {
        if (fixtureFaces) { setFaces([fixtureFace(side)]); setMessage(null); return; }
        if (fixtureNoFaces) { setFaces([]); setMessage("Nenhum rosto encontrado nesta foto."); return; }
        const request = ++generation.current;
        setMessage("Analisando rostos…");
        void detectFaces(event.currentTarget).then((found) => {
          if (generation.current !== request) return;
          setFaces(found);
          setMessage(found.length ? null : "Nenhum rosto encontrado nesta foto.");
        }).catch(() => { if (generation.current === request) setMessage("Não foi possível analisar esta foto. Tente outra."); });
      }
    }} /> : <p className="eye-correction__empty">Prévia indisponível.</p>}
    {choosing && faces.length > 0 && imageWidth > 0 && imageHeight > 0 &&
      <div className="eye-correction__face-layer" style={{ left: size.width / 2 - zoomedWidth / 2 + pan.x, top: size.height / 2 - zoomedHeight / 2 + pan.y, width: zoomedWidth, height: zoomedHeight }}>
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
            onClick={(event) => { if (suppressFaceClick.current === event.currentTarget) { suppressFaceClick.current = null; return; } onChoose(index, faces); }} />;
        })}
      </div>}
    {navigation && <>
      <ImageToolButton label="Referência anterior" icon={ChevronLeft} glyph="navigation" className="eye-correction__nav--previous" disabled={!navigation.previous} onClick={() => navigation.onNavigate(-1)} />
      <ImageToolButton label="Próxima referência" icon={ChevronRight} glyph="navigation" className="eye-correction__nav--next" disabled={!navigation.next} onClick={() => navigation.onNavigate(1)} />
    </>}
    {zoom > 1 && <ImageToolButton label={`Ajustar ${side === "reference" ? "referência" : "imagem a corrigir"} à janela`} icon={Scan} className="eye-correction__fit" onClick={fit} />}
  </div>;
}

export function EyeCorrectionView({ presentation, onNavigate, onCorrection }: Props) {
  const correction = presentation.correction!;
  const [targetIndex, setTargetIndex] = useState<number | null>(null);
  const [referenceIndex, setReferenceIndex] = useState<number | null>(null);
  const [targetFaces, setTargetFaces] = useState<Face[]>([]);
  const [referenceFaces, setReferenceFaces] = useState<Face[]>([]);
  const [targetStatus, setTargetStatus] = useState<string | null>(null);
  const [referenceStatus, setReferenceStatus] = useState<string | null>(null);
  const [comparison, setComparison] = useState<{ key: string; original: boolean } | null>(null);
  const [saveRequested, setSaveRequested] = useState(false);
  const savePending = useRef(false);
  const hintRef = useRef<HTMLParagraphElement>(null);
  const hintTooltip = useUiAnchoredTooltip(hintRef, correction.error ?? "", !correction.error);
  useEffect(() => { setReferenceIndex(null); setReferenceFaces([]); }, [correction.referenceMediaId, correction.phase === "browse"]);
  useEffect(() => { savePending.current = false; setSaveRequested(false); }, [correction.phase, correction.referenceMediaId, correction.resultUrl, correction.error]);
  const referenceChoosing = correction.phase === "select";
  const targetChoosing = correction.phase === "browse" || correction.phase === "select";
  const compareKey = `${correction.phase}:${correction.referenceMediaId ?? ""}:${correction.resultUrl ?? ""}`;
  const showOriginal = correction.phase === "preview" && comparison?.key === compareKey && comparison.original;
  const busy = correction.phase === "applying";
  const action = (kind: ViewerCorrectionAction["kind"]) => onCorrection({ sessionId: presentation.sessionId, kind });
  return <div className="eye-correction" aria-label="Correção de olhos">
    <div className="eye-correction__tools">
      <ImageToolButton label="Fechar correção" icon={EyeOff} tooltipPlacement="bottom" className="eye-correction__close" disabled={busy} onClick={() => action("cancel")} />
      <div className="eye-correction__tool-extension">
        {correction.phase === "browse"
          ? <ImageToolButton label="Usar esta foto" icon={Check} tooltipPlacement="bottom" disabled={!correction.referenceUrl || correction.referenceState !== "ready"} onClick={() => action("select")} />
          : <ImageToolButton label="Trocar referência" icon={RefreshCcw} tooltipPlacement="bottom" disabled={busy} onClick={() => action("browse")} />}
        {correction.phase === "select" && <ImageToolButton label="Ver correção" icon={Sparkles} tooltipPlacement="bottom" disabled={targetIndex === null || referenceIndex === null}
          onClick={() => onCorrection({ sessionId: presentation.sessionId, kind: "preview", referenceMediaId: correction.referenceMediaId, targetFace: targetFaces[targetIndex!], referenceFace: referenceFaces[referenceIndex!] })} />}
        {(correction.phase === "preview" || busy) && <>
          <ImageToolButton label={showOriginal ? "Antes e depois: mostrar correção" : "Antes e depois: mostrar original"} icon={Columns2} tooltipPlacement="bottom" aria-pressed={showOriginal} disabled={busy || !correction.resultUrl}
            onClick={() => setComparison({ key: compareKey, original: !showOriginal })} />
          <ImageToolButton label="Salvar correção" icon={Save} tooltipPlacement="bottom" disabled={busy || saveRequested || !correction.resultUrl} onClick={() => {
            if (savePending.current) return;
            savePending.current = true;
            setSaveRequested(true);
            action("apply");
          }} />
        </>}
      </div>
    </div>
    <p {...hintTooltip.triggerProps} ref={hintRef} className="eye-correction__hint" role="status" data-no-faces={targetStatus === "Nenhum rosto encontrado nesta foto." && referenceStatus === "Nenhum rosto encontrado nesta foto."} tabIndex={correction.error ? 0 : undefined}>{correction.error ?? (
      correction.phase === "processing" ? "Preparando correção…" :
      correction.phase === "applying" ? "Aplicando correção…" : targetStatus ?? referenceStatus ?? ""
    )}</p>
    {hintTooltip.tooltip}
    <div className="eye-correction__panes">
      <div className="eye-correction__pane">
        <FaceImage side="reference" url={correction.referenceUrl} name={correction.referenceName} choosing={referenceChoosing}
          selected={referenceIndex} onChoose={(index, faces) => { setReferenceIndex(index); setReferenceFaces(faces); }} onStatus={setReferenceStatus}
          navigation={correction.phase === "browse" ? { previous: correction.canPreviousReference, next: correction.canNextReference, onNavigate } : undefined} />
      </div>
      <div className="eye-correction__pane">
        <FaceImage side="target" url={(correction.phase === "preview" || correction.phase === "applying") && correction.resultUrl && !showOriginal ? correction.resultUrl : presentation.url}
          name={presentation.name} choosing={targetChoosing} selected={targetIndex}
          onChoose={(index, faces) => { setTargetIndex(index); setTargetFaces(faces); }} onStatus={setTargetStatus} />
      </div>
    </div>
  </div>;
}
