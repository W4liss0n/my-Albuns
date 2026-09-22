import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { ImageViewer } from "../components/ImageViewer";
import type { ViewerPresentation } from "../application/imageViewerWindow";
import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import { tauriImageViewerClient } from "./platform/tauriImageViewerClient";
import { ApplicationHeader, WindowControlsProvider } from "../ui";
import "../ui/theme.css";
import "../ui/ui.css";
import "./viewerWindow.css";

const preview = new URLSearchParams(location.search).get("preview");
if (!preview) installDesktopWebViewPolicy(document);

function ViewerWindow() {
  const [presentation, setPresentation] = useState<ViewerPresentation | null>(() => preview ? {
    sessionId: "preview", revision: 0, mediaId: "preview", name: preview === "long" ? "Serra ao amanhecer com todos os detalhes de uma longa viagem de família.jpg" : "Serra ao amanhecer.jpg",
    url: preview === "missing" ? null : new URL("../test/dev-media/serra-amanhecer.svg", import.meta.url).href,
    state: preview === "missing" ? "absent" : "ready", canPrevious: true, canNext: true,
  } : null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (preview) return;
    let active = true;
    let stop: (() => void) | undefined;
    void tauriImageViewerClient.onPresentation((next) => {
      if (active) setPresentation((current) => current && current.sessionId === next.sessionId && current.revision > next.revision ? current : next);
    }).then(async (dispose) => {
      if (!active) { dispose(); return; }
      stop = dispose;
      try {
        const initial = await tauriImageViewerClient.current();
        if (active) setPresentation(initial);
      } catch { if (active) setError("O estado desta imagem não está disponível."); }
      const token = Number(new URLSearchParams(location.search).get("ownedReadyToken"));
      if (Number.isSafeInteger(token) && token > 0) {
        try { await tauriImageViewerClient.ready(token); }
        catch { if (active) setError("Não foi possível abrir a janela."); }
      }
    });
    return () => { active = false; stop?.(); };
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
        if (preview) { setPresentation((current) => current ? { ...current, mediaId: offset < 0 ? "previous" : "next", name: offset < 0 ? "Retrato.jpg" : "Praia.jpg", url: new URL(offset < 0 ? "../test/dev-media/retrato.svg" : "../test/dev-media/praia.svg", import.meta.url).href } : current); return; }
        void tauriImageViewerClient.navigate(presentation.sessionId, offset).catch(() => undefined);
      }} onClose={close} /> : <div className="image-viewer-window__empty" role="status">{error ?? "Carregando imagem…"}</div>}
    </div>
  </WindowControlsProvider>;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<React.StrictMode><ViewerWindow /></React.StrictMode>);
