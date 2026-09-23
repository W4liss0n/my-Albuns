import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Eye, Scan } from "lucide-react";
import type { ViewerCorrectionAction, ViewerPresentation } from "../application/imageViewerWindow";
import { EyeCorrectionView } from "./EyeCorrectionView";
import { ImageToolButton } from "../ui/ImageToolButton";
import "./ImageViewer.css";

interface Props {
  presentation: ViewerPresentation;
  onNavigate(offset: -1 | 1): void;
  onClose(): void;
  onCorrection?(action: ViewerCorrectionAction): void;
}

const MIN_ZOOM = 1;
const MAX_ZOOM = 8;

export function ImageViewer({ presentation, onNavigate, onClose, onCorrection }: Props) {
  const viewerRef = useRef<HTMLElement>(null);
  const wasCorrecting = useRef(Boolean(presentation.correction));
  const viewportRef = useRef<HTMLDivElement>(null);
  const { mediaId, name, url, state, canPrevious, canNext } = presentation;
  const imageKey = `${mediaId}:${url ?? ""}`;
  const [loaded, setLoaded] = useState<{ key: string; width: number; height: number } | null>(null);
  const [failedKey, setFailedKey] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [viewport, setViewport] = useState({ width: 0, height: 0, fitWidth: 0, fitHeight: 0 });
  const drag = useRef<{ x: number; y: number; panX: number; panY: number; pointerId: number } | null>(null);
  const ready = loaded?.key === imageKey && failedKey !== imageKey;

  useLayoutEffect(() => {
    const correcting = Boolean(presentation.correction);
    if (wasCorrecting.current !== correcting) {
      viewerRef.current?.querySelector<HTMLButtonElement>(correcting ? ".eye-correction__close" : ".image-viewer__eye-action")?.focus();
      wasCorrecting.current = correcting;
    }
  }, [presentation.correction]);

  useLayoutEffect(() => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
    drag.current = null;
  }, [mediaId]);
  useLayoutEffect(() => {
    const element = viewportRef.current;
    if (!element) return;
    const update = () => {
      const style = getComputedStyle(element);
      const width = element.clientWidth;
      const height = element.clientHeight;
      const next = {
        width, height,
        fitWidth: Math.max(0, width - (parseFloat(style.paddingLeft) || 0) - (parseFloat(style.paddingRight) || 0)),
        fitHeight: Math.max(0, height - (parseFloat(style.paddingTop) || 0) - (parseFloat(style.paddingBottom) || 0)),
      };
      setViewport((previous) => previous.width === width && previous.height === height && previous.fitWidth === next.fitWidth && previous.fitHeight === next.fitHeight ? previous : next);
    };
    update();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      update();
      setZoom(1);
      setPan({ x: 0, y: 0 });
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const fittedScale = ready && viewport.fitWidth && viewport.fitHeight
    ? Math.min(1, viewport.fitWidth / loaded.width, viewport.fitHeight / loaded.height) : 0;
  const fittedWidth = ready ? loaded.width * fittedScale : 0;
  const fittedHeight = ready ? loaded.height * fittedScale : 0;
  function boundedPan(next: { x: number; y: number }, nextZoom: number) {
    const maxX = Math.max(0, (fittedWidth * nextZoom - viewport.width) / 2);
    const maxY = Math.max(0, (fittedHeight * nextZoom - viewport.height) / 2);
    return { x: Math.max(-maxX, Math.min(maxX, next.x)), y: Math.max(-maxY, Math.min(maxY, next.y)) };
  }
  function changeZoom(nextZoom: number, clientX?: number, clientY?: number) {
    const value = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, nextZoom));
    const rect = viewportRef.current?.getBoundingClientRect();
    const anchorX = rect && clientX !== undefined ? clientX - rect.left - rect.width / 2 : 0;
    const anchorY = rect && clientY !== undefined ? clientY - rect.top - rect.height / 2 : 0;
    const factor = value / zoom;
    setPan(boundedPan({ x: anchorX - (anchorX - pan.x) * factor, y: anchorY - (anchorY - pan.y) * factor }, value));
    setZoom(value);
  }
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); if (presentation.correction?.phase === "applying") return; if (presentation.correction && onCorrection) onCorrection({ sessionId: presentation.sessionId, kind: "cancel" }); else onClose(); return; }
      if (event.altKey || event.ctrlKey || event.metaKey || event.repeat) return;
      if (presentation.correction) {
        if (presentation.correction.phase === "browse" && event.key === "ArrowLeft" && presentation.correction.canPreviousReference) { event.preventDefault(); onNavigate(-1); }
        else if (presentation.correction.phase === "browse" && event.key === "ArrowRight" && presentation.correction.canNextReference) { event.preventDefault(); onNavigate(1); }
        return;
      }
      if (event.key === "ArrowLeft" && canPrevious) { event.preventDefault(); onNavigate(-1); }
      else if (event.key === "ArrowRight" && canNext) { event.preventDefault(); onNavigate(1); }
      else if (event.key === "+" || event.key === "=") { event.preventDefault(); changeZoom(zoom * 1.25); }
      else if (event.key === "-") { event.preventDefault(); changeZoom(zoom / 1.25); }
      else if (event.key === "0") { event.preventDefault(); setZoom(1); setPan({ x: 0, y: 0 }); }
    };
    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  });
  const message = state === "absent" ? "Imagem não encontrada. Localize o arquivo pelo Painel de imagens."
    : state === "unavailable" ? "Imagem indisponível. Tente novamente pelo Painel de imagens."
    : state === "cache_unavailable" || state === "cache_paused" ? "Prévia temporariamente indisponível."
    : failedKey === imageKey ? "Não foi possível exibir a prévia desta imagem."
    : "Carregando imagem…";

  if (presentation.correction && onCorrection) return <section ref={viewerRef} aria-label="Visualizador de imagens" className="image-viewer"><EyeCorrectionView presentation={presentation} onNavigate={onNavigate} onCorrection={onCorrection} /></section>;

  return <section ref={viewerRef} aria-label="Visualizador de imagens" className="image-viewer">
      <div className="image-viewer__stage" data-zoomed={zoom > 1} ref={viewportRef}
        onWheel={(event) => { event.preventDefault(); if (ready && event.deltaY) changeZoom(zoom * Math.exp(-Math.max(-600, Math.min(600, event.deltaY)) * 0.002), event.clientX, event.clientY); }}
        onPointerDown={(event) => { if (event.button !== 0 || zoom <= 1 || !ready || (event.target as Element).closest("button")) return; drag.current = { x: event.clientX, y: event.clientY, panX: pan.x, panY: pan.y, pointerId: event.pointerId }; event.currentTarget.setPointerCapture(event.pointerId); }}
        onPointerMove={(event) => { const current = drag.current; if (!current || event.pointerId !== current.pointerId) return; setPan(boundedPan({ x: current.panX + event.clientX - current.x, y: current.panY + event.clientY - current.y }, zoom)); }}
        onPointerUp={(event) => { if (drag.current?.pointerId === event.pointerId) { drag.current = null; event.currentTarget.releasePointerCapture(event.pointerId); } }}
        onPointerCancel={() => { drag.current = null; }}>
        {url && <img alt={name} className="image-viewer__image" draggable={false} key={imageKey} src={url}
          style={{ width: fittedWidth, height: fittedHeight, opacity: ready ? 1 : 0, transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})` }}
          onLoad={(event) => { setLoaded({ key: imageKey, width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight }); setFailedKey(null); }}
          onError={() => { setFailedKey(imageKey); setLoaded(null); }} />}
        {!ready && <p aria-live="polite" className="image-viewer__message" role="status">{message}</p>}
        {ready && state !== "ready" && <p className="image-viewer__stale" role="status">Prévia anterior · imagem indisponível</p>}
        <ImageToolButton label="Imagem anterior" icon={ChevronLeft} glyph="navigation" className="image-viewer__nav--previous" disabled={!canPrevious} onClick={() => onNavigate(-1)} />
        <ImageToolButton label="Próxima imagem" icon={ChevronRight} glyph="navigation" className="image-viewer__nav--next" disabled={!canNext} onClick={() => onNavigate(1)} />
        {ready && state === "ready" && onCorrection && <ImageToolButton label="Abrir olhos" icon={Eye} className="image-viewer__eye-action" onClick={() => onCorrection({ sessionId: presentation.sessionId, kind: "start" })} />}
        {zoom > 1 && <ImageToolButton label="Ajustar à janela" icon={Scan} className="image-viewer__fit" onClick={() => { setZoom(1); setPan({ x: 0, y: 0 }); }} />}
      </div>
  </section>;
}
