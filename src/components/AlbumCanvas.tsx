import { useEffect, useRef, useState } from "react";
import { Application } from "pixi.js";

import {
  createLogInstanceId,
  logReasonFromError,
} from "../application/logging";
import type { GraphicsDiagnostic } from "../application/graphics";
import { AlbumCanvasScene } from "./albumCanvasScene";
import type { AlbumCanvasProps } from "./albumCanvasContract";
import {
  useCanvasGraphicsDiagnosticProbe,
} from "./canvasGraphicsDiagnosticProbeContext";
import { useLogger } from "./loggingContext";
import "./pixiRuntime";

const isOpaqueCachePreview = (url: string) =>
  url.startsWith("http://myalbuns-cache.localhost/") ||
  url.startsWith("myalbuns-cache://localhost/");

export type {
  AlbumCanvasProps,
  CanvasMetrics,
  PhotoTransformDelta,
  PhotoTransformPreview,
  PhotoZoomPreview,
} from "./albumCanvasContract";

export function AlbumCanvas(props: AlbumCanvasProps) {
  const logger = useLogger();
  const canvasGraphicsDiagnosticProbe =
    useCanvasGraphicsDiagnosticProbe();
  const latestPropsRef = useRef(props);
  latestPropsRef.current = props;
  const hostRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<AlbumCanvasScene | null>(null);
  const sceneInstanceIdRef = useRef<string | null>(null);
  const materializedSceneRef = useRef<AlbumCanvasScene | null>(null);
  const tracedPreviewUrlsRef = useRef(new Set<string>());
  const [graphicsState, setGraphicsState] = useState<
    "initializing" | "ready" | "recovering" | "failed"
  >("initializing");
  const ready = graphicsState === "ready";
  const [, setPreviewTextureRevision] = useState(0);
  const hasSheets = props.composition.sheets.length > 0;

  useEffect(() => {
    if (!hostRef.current || !hasSheets) return;
    let disposed = false;
    let initialized = false;
    let destroyed = false;
    let ownedScene: AlbumCanvasScene | null = null;
    let contextListenersAttached = false;
    let restoreTimeout: number | null = null;
    let activeDiagnostic: GraphicsDiagnostic | null = null;
    const instanceId = createLogInstanceId("canvas");
    const app = new Application();
    tracedPreviewUrlsRef.current.clear();
    setGraphicsState("initializing");
    logger.write({
      level: "debug",
      component: "canvas",
      event: "canvas_initialization_started",
      projectId: props.projectId,
      instanceId,
      sheetCount: props.composition.sheets.length,
    });
    const clearRestoreTimeout = () => {
      if (restoreTimeout === null) return;
      window.clearTimeout(restoreTimeout);
      restoreTimeout = null;
    };
    const removeContextListeners = () => {
      if (!contextListenersAttached) return;
      contextListenersAttached = false;
      app.canvas.removeEventListener(
        "webglcontextlost",
        handleContextLost,
      );
      app.canvas.removeEventListener(
        "webglcontextrestored",
        handleContextRestored,
      );
    };
    const failCanvas = (diagnostic: GraphicsDiagnostic) => {
      if (disposed) return;
      setGraphicsState("failed");
      latestPropsRef.current.onGraphicsUnavailable?.(diagnostic);
    };
    const handleContextLost = (event: Event) => {
      event.preventDefault();
      if (disposed) return;
      clearRestoreTimeout();
      ownedScene?.suspendForContextLoss();
      setGraphicsState("recovering");
      logger.write({
        level: "warn",
        component: "canvas",
        event: "canvas_context_lost",
        projectId: latestPropsRef.current.projectId,
        instanceId,
      });
      restoreTimeout = window.setTimeout(() => {
        const diagnostic: GraphicsDiagnostic = {
          supported: false,
          code: "context_restore_failed",
          renderer: activeDiagnostic?.renderer ?? "não confirmado",
          reason:
            "O contexto WebGL2 foi perdido e não pôde ser restaurado.",
          limits: activeDiagnostic?.limits ?? null,
        };
        logger.write({
          level: "error",
          component: "canvas",
          event: "canvas_context_restore_failed",
          projectId: latestPropsRef.current.projectId,
          instanceId,
          reason: diagnostic.code,
        });
        failCanvas(diagnostic);
      }, 10_000);
    };
    const handleContextRestored = () => {
      if (disposed) return;
      app.ticker.addOnce(() => {
        if (disposed || !ownedScene || !hostRef.current) return;
        clearRestoreTimeout();
        ownedScene.update(
          latestPropsRef.current,
          hostRef.current.clientHeight,
        );
        setPreviewTextureRevision((current) => current + 1);
        setGraphicsState("ready");
        logger.write({
          level: "info",
          component: "canvas",
          event: "canvas_context_restored",
          projectId: latestPropsRef.current.projectId,
          instanceId,
        });
      });
    };
    const destroyInitializedApp = (reason: string) => {
      if (!initialized || destroyed) return;
      destroyed = true;
      clearRestoreTimeout();
      removeContextListeners();
      const hadScene = ownedScene !== null;
      ownedScene?.destroy();
      if (sceneRef.current === ownedScene) {
        sceneRef.current = null;
        sceneInstanceIdRef.current = null;
        materializedSceneRef.current = null;
      }
      ownedScene = null;
      app.destroy(true, { children: true });
      logger.write({
        level: "debug",
        component: "canvas",
        event: hadScene
          ? "canvas_scene_disposed"
          : "canvas_initialization_abandoned",
        projectId: props.projectId,
        instanceId,
        reason,
      });
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
          destroyInitializedApp("effect_disposed");
          return;
        }
        activeDiagnostic = canvasGraphicsDiagnosticProbe(app.canvas);
        if (!activeDiagnostic.supported) {
          logger.write({
            level: "error",
            component: "canvas",
            event: "canvas_initialization_failed",
            projectId: props.projectId,
            instanceId,
            reason: activeDiagnostic.code,
          });
          failCanvas(activeDiagnostic);
          destroyInitializedApp("graphics_unavailable");
          return;
        }
        app.canvas.addEventListener(
          "webglcontextlost",
          handleContextLost,
        );
        app.canvas.addEventListener(
          "webglcontextrestored",
          handleContextRestored,
        );
        contextListenersAttached = true;
        app.canvas.className = "pixi-canvas";
        app.canvas.setAttribute(
          "aria-label",
          "Canvas contínuo do Álbum. Use a roda para navegar e Alt mais roda para ajustar a Foto.",
        );
        app.canvas.tabIndex = 0;
        hostRef.current.appendChild(app.canvas);
        ownedScene = new AlbumCanvasScene(
          app,
          () => {
            logger.write({
              level: "warn",
              component: "canvas",
              event: "canvas_texture_load_failed",
              projectId: props.projectId,
              instanceId,
              reason: "asset_load_failed",
            });
          },
          () => setPreviewTextureRevision((current) => current + 1),
          (url) => {
            if (tracedPreviewUrlsRef.current.has(url)) return;
            tracedPreviewUrlsRef.current.add(url);
            logger.write({
              level: isOpaqueCachePreview(url) ? "info" : "warn",
              component: "canvas",
              event: isOpaqueCachePreview(url)
                ? "canvas_opaque_preview_texture_loaded"
                : "canvas_preview_texture_transport_rejected",
              projectId: latestPropsRef.current.projectId,
              instanceId,
            });
          },
        );
        sceneRef.current = ownedScene;
        sceneInstanceIdRef.current = instanceId;
        logger.write({
          level: "info",
          component: "canvas",
          event: "canvas_initialization_completed",
          projectId: props.projectId,
          instanceId,
          width: app.screen.width,
          height: app.screen.height,
          sheetCount: props.composition.sheets.length,
        });
        setGraphicsState("ready");
      })
      .catch((error: unknown) => {
        if (!disposed) {
          const diagnostic: GraphicsDiagnostic = {
            supported: false,
            code: "canvas_initialization_failed",
            renderer: "não confirmado",
            reason:
              "Não foi possível inicializar o Canvas WebGL2 do editor.",
            limits: null,
          };
          logger.write({
            level: "error",
            component: "canvas",
            event: "canvas_initialization_failed",
            projectId: props.projectId,
            instanceId,
            reason: logReasonFromError(error),
          });
          failCanvas(diagnostic);
        }
      });

    return () => {
      disposed = true;
      setGraphicsState("initializing");
      destroyInitializedApp("effect_cleanup");
    };
  }, [canvasGraphicsDiagnosticProbe, hasSheets, logger]);

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
    if (materializedSceneRef.current !== scene) {
      materializedSceneRef.current = scene;
      logger.write({
        level: "info",
        component: "canvas",
        event: "canvas_scene_materialized",
        projectId: props.projectId,
        instanceId: sceneInstanceIdRef.current ?? undefined,
        width: host.clientWidth,
        height: host.clientHeight,
        sheetCount: props.composition.sheets.length,
      });
    }
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
      {graphicsState === "initializing" && (
        <span className="canvas-loading">Iniciando WebGL2…</span>
      )}
      {graphicsState === "recovering" && (
        <span className="canvas-loading" role="status">
          Restaurando o contexto gráfico…
        </span>
      )}
      {graphicsState === "failed" && (
        <span className="canvas-loading" role="alert">
          O editor gráfico está indisponível.
        </span>
      )}
    </div>
  );
}
