import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { ImageViewer } from "../components/ImageViewer";
import type { ViewerPresentation } from "../application/imageViewerWindow";
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
const qaError = import.meta.env.DEV && new URLSearchParams(location.search).has("error");
const qaTarget = "/.scratch/eye-correction/qa/nikki-closed.jpg";
const qaReference = "/.scratch/eye-correction/qa/nikki-open-a.jpg";
const previewCorrection = (phase: string) => ({
  phase, referenceMediaId: "reference", referenceName: "Referência.jpg",
  referenceUrl: qa ? qaReference : sizedPreview(portraitPreview, 800, 1200),
  referenceState: "ready" as const, canPreviousReference: false, canNextReference: phase === "browse",
  resultUrl: !qaError && (phase === "preview" || phase === "applying") ? qa ? "/.scratch/eye-correction/qa/nikki-corrected.png" : sizedPreview(landscapePreview, 1200, 800) : null,
  error: qaError ? "Não foi possível preparar a correção para esta fotografia. Escolha outra referência e tente novamente." : null,
});
if (!preview) installDesktopWebViewPolicy(document);
const sizedPreview = (svg: string, width: number, height: number) =>
  `data:image/svg+xml,${encodeURIComponent(svg.replace("<svg ", `<svg width="${width}" height="${height}" `))}`;

function ViewerWindow() {
  const [presentation, setPresentation] = useState<ViewerPresentation | null>(() => preview ? {
    sessionId: "preview", revision: 0, mediaId: "preview", name: qa ? "Nikki — olhos fechados.jpg" : preview === "long" ? "Serra ao amanhecer com todos os detalhes de uma longa viagem de família.jpg" : "Serra ao amanhecer.jpg",
    url: preview === "missing" ? null : qa ? qaTarget : sizedPreview(landscapePreview, 1200, 800),
    state: preview === "missing" ? "absent" : "ready", canPrevious: true, canNext: true,
    correction: preview?.startsWith("correction-") ? previewCorrection(preview.slice("correction-".length)) : undefined,
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
      <ApplicationHeader showBrand={false} controls="maximize-close" context={presentation?.name ?? "Imagem"} />
      {presentation ? <ImageViewer presentation={presentation} onNavigate={(offset) => {
        if (preview) { setPresentation((current) => current ? { ...current, mediaId: offset < 0 ? "previous" : "next", name: offset < 0 ? "Retrato.jpg" : "Praia.jpg", url: offset < 0 ? sizedPreview(portraitPreview, 800, 1200) : new URL("../test/dev-media/praia.svg", import.meta.url).href } : current); return; }
        void tauriImageViewerClient.navigate(presentation.sessionId, offset).catch(() => undefined);
      }} onClose={close} onCorrection={(action) => {
        if (preview) {
          setPresentation((current) => current ? { ...current, correction: action.kind === "cancel" ? undefined
            : action.kind === "start" ? previewCorrection("browse")
            : action.kind === "browse" ? previewCorrection("browse")
            : action.kind === "select" ? previewCorrection("select")
            : action.kind === "preview" ? previewCorrection("preview")
            : action.kind === "apply" ? previewCorrection("applying")
            : current.correction } : current);
          return;
        }
        void tauriImageViewerClient.correction(action).catch(() => undefined);
      }} /> : <div className="image-viewer-window__empty" role="status">{error ?? "Carregando imagem…"}</div>}
    </div>
  </WindowControlsProvider>;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<React.StrictMode><ViewerWindow /></React.StrictMode>);
