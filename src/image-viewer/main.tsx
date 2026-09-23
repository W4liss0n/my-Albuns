import React, { useCallback, useEffect, useRef, useState } from "react";
import ReactDOM from "react-dom/client";
import { ImageViewer } from "../components/ImageViewer";
import type { ViewerPresentation } from "../application/imageViewerWindow";
import type { ViewerCorrectionPhase } from "../contracts/generated/ViewerCorrectionPhase";
import { observeViewerPresentation } from "./viewerObservation";
import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import { tauriImageViewerClient } from "./platform/tauriImageViewerClient";
import { ApplicationHeader, WindowControlsProvider } from "../ui";
import landscapePreview from "../test/dev-media/serra-amanhecer.svg?raw";
import portraitPreview from "../test/dev-media/retrato.svg?raw";
import "../ui/theme.css";
import "../ui/ui.css";
import "./viewerWindow.css";

const preview = new URLSearchParams(location.search).get("preview");
const qa = import.meta.env.DEV && new URLSearchParams(location.search).has("qa");
const qaError = import.meta.env.DEV ? new URLSearchParams(location.search).get("error") : null;
const qaMultipleFaces = import.meta.env.DEV && new URLSearchParams(location.search).get("fixture") === "faces-multi";
const qaDelayedPreparation = import.meta.env.DEV && new URLSearchParams(location.search).get("delay") === "prepare";
const qaTarget = qaMultipleFaces ? "/.scratch/face-detection-debug-20260923/inputs/IMG_6246.JPG" : "/.scratch/eye-correction/qa/nikki-closed.jpg";
const qaReference = qaMultipleFaces ? "/.scratch/face-detection-debug-20260923/inputs/IMG_6252.JPG" : "/.scratch/eye-correction/qa/nikki-open-a.jpg";
const qaReferences = qaMultipleFaces
  ? ["IMG_6252.JPG", "IMG_6276.JPG", "IMG_6300.JPG"].map((name) => ({ name, url: `/.scratch/face-detection-debug-20260923/inputs/${name}` }))
  : [{ name: "Referência.jpg", url: qaReference }, { name: "Outra referência.jpg", url: "/.scratch/face-detection-debug-20260923/inputs/reference.jpg" }];
const referenceIndex = (correction: ViewerPresentation["correction"]) => Number(correction?.referenceMediaId?.split("-")[1] ?? 0);
const previewPhase = (value: string | null): ViewerCorrectionPhase | null => {
  switch (value) {
    case "browse": case "select": case "processing": case "preview": case "applying": return value;
    default: return null;
  }
};
const initialPreviewPhase = preview?.startsWith("correction-") ? previewPhase(preview.slice("correction-".length)) : null;
const previewCorrection = (phase: ViewerCorrectionPhase, version = 0, index = 0): NonNullable<ViewerPresentation["correction"]> => ({
  phase, referenceMediaId: `reference-${index}`, referenceName: qa ? qaReferences[index].name : "Referência.jpg",
  referenceUrl: qa ? qaReferences[index].url : sizedPreview(portraitPreview, 800, 1200),
  referenceState: "ready" as const, canPreviousReference: phase === "browse" && index > 0, canNextReference: phase === "browse" && index < qaReferences.length - 1,
  resultUrl: phase === "preview" || phase === "applying" ? qaMultipleFaces ? `${qaTarget}?v=${version}` : qa ? `/.scratch/eye-correction/qa/nikki-corrected.png${version ? `?v=${version}` : ""}` : sizedPreview(landscapePreview, 1200, 800) : null,
  error: qaError === "prepare" && phase === "select" ? "Os olhos da referência precisam estar abertos."
    : qaError === "save" && phase === "preview" ? "Não foi possível atualizar a prévia da foto. A foto original foi restaurada." : null,
});
if (!preview) installDesktopWebViewPolicy(document);
const sizedPreview = (svg: string, width: number, height: number) =>
  `data:image/svg+xml,${encodeURIComponent(svg.replace("<svg ", `<svg width="${width}" height="${height}" `))}`;

function ViewerWindow() {
  const qaPreviewRequest = useRef(0);
  const [visibleTitle, setVisibleTitle] = useState<{ sessionId: string; name: string } | null>(null);
  const onVisibleNameChange = useCallback((sessionId: string, name: string) => {
    setVisibleTitle((current) => current?.sessionId === sessionId && current.name === name ? current : { sessionId, name });
  }, []);
  const [presentation, setPresentation] = useState<ViewerPresentation | null>(() => preview ? {
    sessionId: "preview", revision: 0, mediaId: "preview", name: qaMultipleFaces ? "IMG_6246.JPG" : qa ? "Nikki — olhos fechados.jpg" : preview === "long" ? "Serra ao amanhecer com todos os detalhes de uma longa viagem de família.jpg" : "Serra ao amanhecer.jpg",
    url: preview === "missing" ? null : qa ? qaTarget : sizedPreview(landscapePreview, 1200, 800),
    state: preview === "missing" ? "absent" : "ready", canPrevious: true, canNext: true,
    correction: initialPreviewPhase ? previewCorrection(initialPreviewPhase) : undefined,
  } : null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (preview) return;
    let active = true;
    const observation = observeViewerPresentation(tauriImageViewerClient, setPresentation);
    void observation.ready.then(async () => {
      if (!active) return;
      const token = Number(new URLSearchParams(location.search).get("ownedReadyToken"));
      if (Number.isSafeInteger(token) && token > 0) {
        try { await tauriImageViewerClient.ready(token); }
        catch { if (active) setError("Não foi possível abrir a janela."); }
      }
    }).catch(() => { if (active) setError("O estado desta imagem não está disponível."); });
    return () => { active = false; observation.dispose(); };
  }, []);
  const close = () => {
    if (presentation?.correction?.phase === "applying" || preview) return;
    if (presentation) void tauriImageViewerClient.close(presentation.sessionId).catch(() => undefined);
  };
  const controls = { ...tauriWindowControls, close };
  return <WindowControlsProvider controls={controls}>
    <div className="image-viewer-window">
      <ApplicationHeader showBrand={false} controls="maximize-close" context={presentation?.correction ? "" : (visibleTitle?.sessionId === presentation?.sessionId ? visibleTitle?.name : presentation?.name) ?? "Imagem"} />
      {presentation ? <ImageViewer presentation={presentation} onVisibleNameChange={onVisibleNameChange} onNavigate={(offset) => {
        if (preview) { setPresentation((current) => {
          if (!current) return current;
          if (current.correction?.phase === "browse") {
            const index = Math.max(0, Math.min(qaReferences.length - 1, referenceIndex(current.correction) + offset));
            return { ...current, correction: previewCorrection("browse", 0, index) };
          }
          return { ...current, mediaId: offset < 0 ? "previous" : "next", name: offset < 0 ? "Retrato.jpg" : "Praia.jpg", url: offset < 0 ? sizedPreview(portraitPreview, 800, 1200) : new URL("../test/dev-media/praia.svg", import.meta.url).href };
        }); return; }
        void tauriImageViewerClient.navigate(presentation.sessionId, offset).catch(() => undefined);
      }} onClose={close} onCorrection={(action) => {
        if (preview) {
          if (action.kind === "preview") {
            const request = ++qaPreviewRequest.current;
            setPresentation((current) => current ? { ...current, correction: qaDelayedPreparation ? previewCorrection("processing", 0, referenceIndex(current.correction))
              : qaError === "prepare" ? previewCorrection("select", 0, referenceIndex(current.correction)) : previewCorrection("preview", request, referenceIndex(current.correction)) } : current);
            if (qaDelayedPreparation) window.setTimeout(() => {
              if (request !== qaPreviewRequest.current) return;
              setPresentation((current) => current?.correction?.phase === "processing"
                ? { ...current, correction: qaError === "prepare" ? previewCorrection("select", 0, referenceIndex(current.correction)) : previewCorrection("preview", request, referenceIndex(current.correction)) } : current);
            }, 800);
            return;
          }
          if (action.kind === "cancel" || action.kind === "browse" || action.kind === "start" || action.kind === "select") qaPreviewRequest.current++;
          setPresentation((current) => current ? { ...current, correction: action.kind === "cancel" ? undefined
            : action.kind === "start" ? previewCorrection("browse")
            : action.kind === "browse" ? previewCorrection("browse", 0, referenceIndex(current.correction))
            : action.kind === "select" ? previewCorrection("select", 0, referenceIndex(current.correction))
            : action.kind === "apply" ? qaError === "save" ? previewCorrection("preview", qaPreviewRequest.current, referenceIndex(current.correction)) : previewCorrection("applying", qaPreviewRequest.current, referenceIndex(current.correction))
            : current.correction } : current);
          return;
        }
        void tauriImageViewerClient.correction(action).catch(() => undefined);
      }} /> : <div className="image-viewer-window__empty" role="status">{error ?? "Carregando imagem…"}</div>}
    </div>
  </WindowControlsProvider>;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<React.StrictMode><ViewerWindow /></React.StrictMode>);
