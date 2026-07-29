import { useEffect, useRef, useState } from "react";
import { Application } from "pixi.js";

import { AlbumCanvasScene } from "./albumCanvasScene";
import type { AlbumCanvasProps } from "./albumCanvasContract";

export type {
  AlbumCanvasProps,
  CanvasMetrics,
  PhotoTransformDelta,
  PhotoTransformPreview,
  PhotoZoomPreview,
} from "./albumCanvasContract";

export function AlbumCanvas(props: AlbumCanvasProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<AlbumCanvasScene | null>(null);
  const [ready, setReady] = useState(false);
  const hasSheets = props.composition.sheets.length > 0;

  useEffect(() => {
    if (!hostRef.current || !hasSheets) return;
    let disposed = false;
    let initialized = false;
    let destroyed = false;
    const app = new Application();
    const destroyInitializedApp = () => {
      if (!initialized || destroyed) return;
      destroyed = true;
      sceneRef.current?.destroy();
      sceneRef.current = null;
      app.destroy(true, { children: true });
    };

    void app
      .init({
        resizeTo: hostRef.current,
        backgroundAlpha: 0,
        antialias: true,
        autoDensity: true,
        resolution: window.devicePixelRatio,
        preference: "webgl",
        preferWebGLVersion: 2,
        powerPreference: "high-performance",
      })
      .then(() => {
        initialized = true;
        if (disposed || !hostRef.current) {
          destroyInitializedApp();
          return;
        }
        app.canvas.className = "pixi-canvas";
        app.canvas.setAttribute(
          "aria-label",
          "Canvas contínuo do Álbum. Use a roda para navegar e Alt mais roda para ajustar a Foto.",
        );
        app.canvas.tabIndex = 0;
        hostRef.current.appendChild(app.canvas);
        sceneRef.current = new AlbumCanvasScene(app);
        setReady(true);
      })
      .catch((error: unknown) => {
        if (!disposed) {
          console.error("Não foi possível iniciar o Canvas PixiJS.", error);
        }
      });

    return () => {
      disposed = true;
      setReady(false);
      destroyInitializedApp();
    };
  }, [hasSheets]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host || !hasSheets || typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver(() => {
      sceneRef.current?.resize(host.clientHeight);
    });
    observer.observe(host);
    return () => observer.disconnect();
  }, [hasSheets]);

  useEffect(() => {
    const scene = sceneRef.current;
    const host = hostRef.current;
    if (!ready || !scene || !host) return;
    scene.update(props, host.clientHeight);
  });

  if (!hasSheets) {
    return (
      <div className="canvas-empty">
        <p>Nenhuma Lâmina disponível para materialização.</p>
      </div>
    );
  }

  return (
    <div className="canvas-host" ref={hostRef}>
      {!ready && <span className="canvas-loading">Iniciando WebGL2…</span>}
    </div>
  );
}
