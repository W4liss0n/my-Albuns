import { useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Scan } from "lucide-react";
import type { ViewerCorrectionAction, ViewerPresentation } from "../application/imageViewerWindow";
import { detectFaces, faceCenter, type Face } from "../image-viewer/faceLandmarks";
import { ActionButton } from "../ui/ActionButton";
import { ImageToolButton } from "../ui/ImageToolButton";

interface Props {
  presentation: ViewerPresentation;
  onNavigate(offset: -1 | 1): void;
  onCorrection(action: ViewerCorrectionAction): void;
}

type Side = "reference" | "target";
// Visual QA only: stable marker positions over the local QA photos. This does not exercise detection.
const fixtureFaces = import.meta.env.DEV && new URLSearchParams(location.search).get("fixture") === "faces";
function fixtureFace(side: Side): Face {
  const points = Array.from({ length: 264 }, () => ({ x: 0.5, y: 0.4, z: 0 }));
  const values = side === "reference"
    ? [[1, .492, .256], [33, .412, .279], [152, .582, .371], [263, .547, .170]]
    : [[1, .476, .427], [33, .393, .384], [152, .558, .548], [263, .533, .275]];
  for (const [index, x, y] of values) points[index] = { x, y, z: 0 };
  return points;
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
  const drag = useRef<{ x: number; y: number; panX: number; panY: number; id: number } | null>(null);
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
  const bound = (next: { x: number; y: number }, value: number) => ({
    x: Math.max(-Math.max(0, (imageWidth * value - size.width) / 2), Math.min(Math.max(0, (imageWidth * value - size.width) / 2), next.x)),
    y: Math.max(-Math.max(0, (imageHeight * value - size.height) / 2), Math.min(Math.max(0, (imageHeight * value - size.height) / 2), next.y)),
  });
  const fit = () => { setZoom(1); setPan({ x: 0, y: 0 }); };
  return <div className="eye-correction__pane-image" ref={pane} tabIndex={0}
    onWheel={(event) => { if (!url || !event.deltaY) return; event.preventDefault(); const value = Math.max(1, Math.min(8, zoom * Math.exp(-event.deltaY * 0.002))); setPan(bound(pan, value)); setZoom(value); }}
    onKeyDown={(event) => { if (event.key === "0") { event.preventDefault(); fit(); } else if (event.key === "+" || event.key === "=") { event.preventDefault(); const value = Math.min(8, zoom * 1.25); setPan(bound(pan, value)); setZoom(value); } else if (event.key === "-") { event.preventDefault(); const value = Math.max(1, zoom / 1.25); setPan(bound(pan, value)); setZoom(value); } }}
    onPointerDown={(event) => { if (zoom <= 1 || event.button !== 0 || (event.target as Element).closest("button")) return; drag.current = { x: event.clientX, y: event.clientY, panX: pan.x, panY: pan.y, id: event.pointerId }; event.currentTarget.setPointerCapture(event.pointerId); }}
    onPointerMove={(event) => { if (drag.current?.id !== event.pointerId) return; setPan(bound({ x: drag.current.panX + event.clientX - drag.current.x, y: drag.current.panY + event.clientY - drag.current.y }, zoom)); }}
    onPointerUp={(event) => { if (drag.current?.id === event.pointerId) { drag.current = null; event.currentTarget.releasePointerCapture(event.pointerId); } }}
    onPointerCancel={() => { drag.current = null; }}>
    {url ? <img ref={image} alt={name} src={url} crossOrigin="anonymous" draggable={false} style={{ transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})` }} onLoad={(event) => {
      setNatural({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight });
      if (choosing) {
        if (fixtureFaces) { setFaces([fixtureFace(side)]); setMessage(null); return; }
        const request = ++generation.current;
        setMessage("Analisando rostos…");
        void detectFaces(event.currentTarget).then((found) => {
          if (generation.current !== request) return;
          setFaces(found);
          setMessage(found.length ? null : "Nenhum rosto encontrado nesta foto.");
        }).catch(() => { if (generation.current === request) setMessage("Não foi possível analisar esta foto. Tente outra."); });
      }
    }} /> : <p className="eye-correction__empty">Prévia indisponível.</p>}
    {choosing && faces.map((face, index) => {
      const center = faceCenter(face);
      const eyeSpan = face[33] && face[263] ? Math.abs(face[263].x - face[33].x) : 0.1;
      const x = size.width / 2 + ((center.x - 0.5 + eyeSpan * 0.8) * imageWidth) * zoom + pan.x;
      const y = size.height / 2 + ((center.y - 0.5 - eyeSpan * 0.7) * imageHeight) * zoom + pan.y;
      return <button key={index} type="button" className="eye-correction__face" data-selected={selected === index}
        style={{ left: Math.max(21, Math.min(size.width - 21, x)), top: Math.max(21, Math.min(size.height - 21, y)) }}
        aria-pressed={selected === index} aria-label={`${side === "reference" ? "Referência" : "Imagem a corrigir"}: rosto ${index + 1}`}
        onClick={() => onChoose(index, faces)}><span>{index + 1}</span></button>;
    })}
    {navigation && <>
      <ImageToolButton label="Referência anterior" icon={ChevronLeft} className="eye-correction__nav--previous" disabled={!navigation.previous} onClick={() => navigation.onNavigate(-1)} />
      <ImageToolButton label="Próxima referência" icon={ChevronRight} className="eye-correction__nav--next" disabled={!navigation.next} onClick={() => navigation.onNavigate(1)} />
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
  useEffect(() => { setReferenceIndex(null); setReferenceFaces([]); }, [correction.referenceMediaId, correction.phase === "browse"]);
  const referenceChoosing = correction.phase === "select";
  const targetChoosing = correction.phase === "browse" || correction.phase === "select";
  const action = (kind: ViewerCorrectionAction["kind"]) => onCorrection({ sessionId: presentation.sessionId, kind });
  return <div className="eye-correction" aria-label="Correção de olhos">
    <div className="eye-correction__panes">
      <div className="eye-correction__pane">
        <div className="eye-correction__pane-label">Referência <span title={correction.referenceName}>{correction.referenceName}</span></div>
        <FaceImage side="reference" url={correction.referenceUrl} name={correction.referenceName} choosing={referenceChoosing}
          selected={referenceIndex} onChoose={(index, faces) => { setReferenceIndex(index); setReferenceFaces(faces); }} onStatus={setReferenceStatus}
          navigation={correction.phase === "browse" ? { previous: correction.canPreviousReference, next: correction.canNextReference, onNavigate } : undefined} />
        <div className="eye-correction__reference-actions">
          {correction.phase === "browse" && <ActionButton variant="quiet" disabled={!correction.referenceUrl || correction.referenceState !== "ready"} onClick={() => action("select")}>Usar esta foto</ActionButton>}
          {(correction.phase === "select" || correction.phase === "preview") && <ActionButton variant="quiet" onClick={() => action("browse")}>Trocar referência</ActionButton>}
        </div>
      </div>
      <div className="eye-correction__pane">
        <div className="eye-correction__pane-label">Imagem a corrigir <span title={presentation.name}>{presentation.name}</span></div>
        <FaceImage side="target" url={(correction.phase === "preview" || correction.phase === "applying") && correction.resultUrl ? correction.resultUrl : presentation.url}
          name={presentation.name} choosing={targetChoosing} selected={targetIndex}
          onChoose={(index, faces) => { setTargetIndex(index); setTargetFaces(faces); }} onStatus={setTargetStatus} />
        <div className="eye-correction__reference-actions" aria-hidden="true" />
      </div>
    </div>
    <div className="eye-correction__footer">
      <p className="eye-correction__hint" role="status" title={correction.error ?? undefined}>{correction.error ?? (
        correction.phase === "processing" ? "Preparando correção…" :
        correction.phase === "applying" ? "Aplicando correção…" : targetStatus ?? referenceStatus ?? ""
      )}</p>
      <div className="eye-correction__actions">
        <ActionButton variant="quiet" disabled={correction.phase === "applying"} onClick={() => action("cancel")}>Cancelar</ActionButton>
        <span className="eye-correction__confirm-slot">
          {correction.phase === "select" && <ActionButton variant="neutral" disabled={targetIndex === null || referenceIndex === null}
            onClick={() => onCorrection({ sessionId: presentation.sessionId, kind: "preview", referenceMediaId: correction.referenceMediaId, targetFace: targetFaces[targetIndex!], referenceFace: referenceFaces[referenceIndex!] })}>Ver correção</ActionButton>}
          {correction.phase === "preview" && <ActionButton variant="neutral" onClick={() => action("apply")}>Aplicar</ActionButton>}
        </span>
      </div>
    </div>
  </div>;
}
