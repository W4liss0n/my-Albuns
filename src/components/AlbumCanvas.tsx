import { useEffect, useRef, useState } from "react";
import { Application } from "pixi.js";

import {
  createLogInstanceId,
  logReasonFromError,
} from "../application/logging";
import { AlbumCanvasScene } from "./albumCanvasScene";
import type { AlbumCanvasProps } from "./albumCanvasContract";
import { runCanvasPerformanceProbe } from "./canvasPerformanceProbe";
import { useLogger } from "./loggingContext";
import "./pixiRuntime";

export type {
  AlbumCanvasProps,
  CanvasMetrics,
  PhotoTransformDelta,
  PhotoTransformPreview,
  PhotoZoomPreview,
} from "./albumCanvasContract";

export function AlbumCanvas(props: AlbumCanvasProps) {
  const logger = useLogger();
  const hostRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<AlbumCanvasScene | null>(null);
  const sceneInstanceIdRef = useRef<string | null>(null);
  const materializedSceneRef = useRef<AlbumCanvasScene | null>(null);
  const performanceRunRef = useRef<{
    key: string;
    controller: AbortController;
  } | null>(null);
  const [ready, setReady] = useState(false);
  const [, setPreviewTextureRevision] = useState(0);
  const hasSheets = props.composition.sheets.length > 0;

  useEffect(() => {
    if (!hostRef.current || !hasSheets) return;
    let disposed = false;
    let initialized = false;
    let destroyed = false;
    let ownedScene: AlbumCanvasScene | null = null;
    const instanceId = createLogInstanceId("canvas");
    const app = new Application();
    logger.write({
      level: "debug",
      component: "canvas",
      event: "canvas_initialization_started",
      projectId: props.projectId,
      instanceId,
      sheetCount: props.composition.sheets.length,
    });
    const destroyInitializedApp = (reason: string) => {
      if (!initialized || destroyed) return;
      destroyed = true;
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
        setReady(true);
      })
      .catch((error: unknown) => {
        if (!disposed) {
          logger.write({
            level: "error",
            component: "canvas",
            event: "canvas_initialization_failed",
            projectId: props.projectId,
            instanceId,
            reason: logReasonFromError(error),
          });
        }
      });

    return () => {
      disposed = true;
      setReady(false);
      performanceRunRef.current?.controller.abort();
      performanceRunRef.current = null;
      destroyInitializedApp("effect_cleanup");
    };
  }, [hasSheets, logger]);

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

  useEffect(() => {
    const request = props.performanceProbe;
    const scene = sceneRef.current;
    if (!ready || !request || !scene) return;
    if (performanceRunRef.current?.key === request.key) return;
    const targetState = scene.performanceTarget();
    if (targetState.status === "pending") return;

    performanceRunRef.current?.controller.abort();
    const controller = new AbortController();
    performanceRunRef.current = {
      key: request.key,
      controller,
    };
    if (targetState.status === "failed") {
      logger.write({
        level: "error",
        component: "canvas",
        event: "canvas_performance_probe_failed",
        projectId: props.projectId,
        instanceId: sceneInstanceIdRef.current ?? undefined,
        reason: targetState.reason,
      });
      void request.onFailed(targetState.reason);
      return;
    }
    const target = targetState.target;
    logger.write({
      level: "info",
      component: "canvas",
      event: "canvas_performance_probe_started",
      projectId: props.projectId,
      instanceId: sceneInstanceIdRef.current ?? undefined,
    });
    void Promise.resolve(request.onReady())
      .then(() =>
        runCanvasPerformanceProbe(
          request.config,
          target,
          undefined,
          controller.signal,
        ),
      )
      .then(async (measurement) => {
        if (
          controller.signal.aborted ||
          performanceRunRef.current?.controller !== controller
        ) {
          return;
        }
        await request.onCompleted(measurement);
        logger.write({
          level: "info",
          component: "canvas",
          event: "canvas_performance_probe_completed",
          projectId: props.projectId,
          instanceId: sceneInstanceIdRef.current ?? undefined,
        });
      })
      .catch(async (error: unknown) => {
        if (
          controller.signal.aborted ||
          (error instanceof DOMException && error.name === "AbortError")
        ) {
          return;
        }
        const reason = logReasonFromError(error);
        logger.write({
          level: "error",
          component: "canvas",
          event: "canvas_performance_probe_failed",
          projectId: props.projectId,
          instanceId: sceneInstanceIdRef.current ?? undefined,
          reason,
        });
        await request.onFailed(reason);
      });
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
