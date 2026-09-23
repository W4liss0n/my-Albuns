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
if (!preview) installDesktopWebViewPolicy(document);
const sizedPreview = (svg: string, width: number, height: number) =>
  `data:image/svg+xml,${encodeURIComponent(svg.replace("<svg ", `<svg width="${width}" height="${height}" `))}`;

function ViewerWindow() {
  const [presentation, setPresentation] = useState<ViewerPresentation | null>(() => preview ? {
    sessionId: "preview", revision: 0, mediaId: "preview", name: preview === "long" ? "Serra ao amanhecer com todos os detalhes de uma longa viagem de família.jpg" : "Serra ao amanhecer.jpg",
    url: preview === "missing" ? null : sizedPreview(landscapePreview, 1200, 800),
    state: preview === "missing" ? "absent" : "ready", canPrevious: true, canNext: true,
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
    if (preview) return;
    if (presentation) void tauriImageViewerClient.close(presentation.sessionId).catch(() => undefined);
  };
  const controls = { ...tauriWindowControls, close };
  return <WindowControlsProvider controls={controls}>
    <div className="image-viewer-window">
      <ApplicationHeader showBrand={false} controls="maximize-close" context={presentation?.name ?? "Imagem"} />
      {presentation ? <ImageViewer presentation={presentation} onNavigate={(offset) => {
        if (preview) { setPresentation((current) => current ? { ...current, mediaId: offset < 0 ? "previous" : "next", name: offset < 0 ? "Retrato.jpg" : "Praia.jpg", url: offset < 0 ? sizedPreview(portraitPreview, 800, 1200) : new URL("../test/dev-media/praia.svg", import.meta.url).href } : current); return; }
        void tauriImageViewerClient.navigate(presentation.sessionId, offset).catch(() => undefined);
      }} onClose={close} /> : <div className="image-viewer-window__empty" role="status">{error ?? "Carregando imagem…"}</div>}
    </div>
  </WindowControlsProvider>;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<React.StrictMode><ViewerWindow /></React.StrictMode>);
