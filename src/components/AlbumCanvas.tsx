import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
} from "react";
import { Layers3 } from "lucide-react";
import { Application } from "pixi.js";

import {
  createLogInstanceId,
  logReasonFromError,
} from "../application/logging";
import type { GraphicsDiagnostic } from "../application/graphics";
import type { PhotoDropTarget } from "../domain/project";
import { AppIcon, EmptyState } from "../ui";
import { AlbumCanvasScene } from "./albumCanvasScene";
import { useDecorativeDropPreview } from "./useDecorativeDropPreview";
import type {
  AlbumCanvasProps,
  CanvasPhotoDropPoint,
  CanvasMetrics,
} from "./albumCanvasContract";
import { CanvasHorizontalScrollbar } from "./CanvasHorizontalScrollbar";
import { SheetBarOverlay } from "./SheetBarOverlay";
import { albumCanvasModePolicy, sheetsForCanvasMode } from "./albumCanvasMode";
import { createNormalCanvasLayout } from "./canvasSheetViewGeometry";
import { MICROMETER_TO_CANVAS_PIXEL } from "./canvasGeometry";
import {
  useCanvasGraphicsDiagnosticProbe,
} from "./canvasGraphicsDiagnosticProbeContext";
import { useLogger } from "./loggingContext";
import "./AlbumCanvas.css";
import "./pixiRuntime";

const isOpaqueCachePreview = (url: string) =>
  url.startsWith("http://myalbuns-cache.localhost/") ||
  url.startsWith("myalbuns-cache://localhost/");

export type {
  AlbumCanvasMode,
  AlbumCanvasProps,
  CanvasPhotoDropPoint,
  CanvasMetrics,
  CanvasSheetReorder,
  CanvasTechnicalGuides,
  PhotoTransformDelta,
  PhotoTransformPreview,
  PhotoZoomPreview,
  SheetBarMetadata,
} from "./albumCanvasContract";

export function AlbumCanvas(props: AlbumCanvasProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<AlbumCanvasScene | null>(null);
  const decorative = useDecorativeDropPreview({
    projectId: props.projectId, revision: props.revision, drag: props.mediaDrag,
    preview: props.onPreviewDecorativeDrop, commit: props.onDropDecorative, cancel: props.onPhotoDragCancel,
    point: (x, y) => {
      const bounds = hostRef.current?.querySelector("canvas")?.getBoundingClientRect();
      if (!bounds || x < bounds.left || x >= bounds.right || y < bounds.top || y >= bounds.bottom) return null;
      return sceneRef.current?.resolvePhotoDropPoint(x, y) ?? null;
    },
  });
  const displayedComposition = useMemo(() => decorative.preview ? {
    ...props.composition,
    sheets: props.composition.sheets.map((sheet) => sheet.sheetId === decorative.preview!.sheet.sheetId ? decorative.preview!.sheet : sheet),
  } : props.composition, [decorative.preview, props.composition]);
  const [resolvedPhotoDrop, setResolvedPhotoDrop] = useState<{
    request: number;
    mediaId: string;
    point: CanvasPhotoDropPoint;
    target: PhotoDropTarget;
  } | null>(null);
  const logger = useLogger();
  const canvasGraphicsDiagnosticProbe =
    useCanvasGraphicsDiagnosticProbe();
  const externalMetricsCallbackRef = useRef(props.onCanvasMetricsChange);
  externalMetricsCallbackRef.current = props.onCanvasMetricsChange;
  const [canvasMetrics, setCanvasMetrics] = useState<CanvasMetrics | null>(
    null,
  );
  const [sheetAutoScrollVelocity, setSheetAutoScrollVelocity] =
    useState(0);
  const handleCanvasMetricsChange = useCallback((metrics: CanvasMetrics) => {
    setCanvasMetrics((current) =>
      current &&
      Math.abs(current.width - metrics.width) < 0.0001 &&
      Math.abs(current.height - metrics.height) < 0.0001 &&
      Math.abs(current.scale - metrics.scale) < 0.0001
        ? current
        : metrics,
    );
    externalMetricsCallbackRef.current?.(metrics);
  }, []);
  const isolated = props.mode.kind === "normal" && Boolean(props.mode.isolatedSheetId);
  const visibleSheets = sheetsForCanvasMode(props.composition.sheets, albumCanvasModePolicy(props.mode));
  const barLayout = isolated ? createNormalCanvasLayout(visibleSheets, props.technicalGuides?.bleedUm)
    : props.continuousCanvasLayout;
  const barViewport = isolated && canvasMetrics && visibleSheets[0]
    ? { ...props.viewport, offsetX: barLayout.centeredOffset(visibleSheets[0].sheetId, canvasMetrics.scale, canvasMetrics.width) ?? 0 }
    : props.viewport;
  const barOffsetY = isolated && canvasMetrics && visibleSheets[0]
    ? (canvasMetrics.height - visibleSheets[0].heightUm * MICROMETER_TO_CANVAS_PIXEL * canvasMetrics.scale) / 2
    : undefined;
  const doubleSheetIds = new Set(props.composition.sheets
    .filter((sheet) => sheet.activeSides === "both")
    .map((sheet) => sheet.sheetId));
  const sheetBarMetadata = props.sheetBarMetadata.map((metadata) => ({
    ...metadata,
    canSwapSides: props.mode.kind === "normal" &&
      Boolean(props.sheetSideSwap && !props.sheetSideSwap.disabled) &&
      Boolean(props.sheetReorder && !props.sheetReorder.disabled && ["idle", "cancelled"].includes(props.sheetReorder.status)) &&
      !metadata.layoutLocked &&
      doubleSheetIds.has(metadata.sheetId),
  }));
  const sceneProps = {
    ...props,
    composition: displayedComposition,
    decorativeDropPreview: decorative.preview,
    sheetReorder: props.sheetReorder && isolated ? { ...props.sheetReorder, disabled: true } : props.sheetReorder,
    sheetBarMetadata,
    photoDropHighlight: resolvedPhotoDrop?.target ?? null,
    onCanvasMetricsChange: handleCanvasMetricsChange,
  };
  const latestPropsRef = useRef(sceneProps);
  latestPropsRef.current = sceneProps;
  const sceneInstanceIdRef = useRef<string | null>(null);
  const materializedSceneRef = useRef<AlbumCanvasScene | null>(null);
  const tracedPreviewUrlsRef = useRef(new Set<string>());
  const [graphicsState, setGraphicsState] = useState<
    "initializing" | "ready" | "recovering" | "failed"
  >("initializing");
  const ready = graphicsState === "ready";
  const [, setPreviewTextureRevision] = useState(0);
  const hasSheets = props.composition.sheets.length > 0;
  const dragRequestRef = useRef(0);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const handleWheel = (event: WheelEvent) => {
      sceneRef.current?.handleCanvasWheel(event);
    };
    host.addEventListener("wheel", handleWheel, { passive: false });
    return () => host.removeEventListener("wheel", handleWheel);
  }, []);

  useEffect(() => {
    if (sheetAutoScrollVelocity === 0 || !canvasMetrics) return;
    let frame = 0;
    let previousTimestamp = performance.now();
    const advance = (timestamp: number) => {
      const elapsedSeconds =
        Math.min(50, Math.max(0, timestamp - previousTimestamp)) / 1_000;
      previousTimestamp = timestamp;
      const latest = latestPropsRef.current;
      const offsetX = latest.continuousCanvasLayout.clampOffset(
        latest.viewport.offsetX - sheetAutoScrollVelocity * elapsedSeconds,
        canvasMetrics.scale,
        canvasMetrics.width,
      );
      if (offsetX !== latest.viewport.offsetX) {
        latest.onViewportChange({ ...latest.viewport, offsetX });
      }
      frame = requestAnimationFrame(advance);
    };
    frame = requestAnimationFrame(advance);
    return () => cancelAnimationFrame(frame);
  }, [canvasMetrics, sheetAutoScrollVelocity]);

  useEffect(() => {
    if (
      props.mode.kind === "sheet-editing" ||
      !props.sheetReorder ||
      props.sheetReorder.disabled
    ) {
      setSheetAutoScrollVelocity(0);
    }
  }, [props.mode.kind, props.sheetReorder]);

  useEffect(() => {
    const drag = props.mediaDrag;
    const request = ++dragRequestRef.current;
    setResolvedPhotoDrop(null);
    if (!drag || drag.kind !== "photo") return;
    const bounds = hostRef.current?.querySelector("canvas")?.getBoundingClientRect();
    const inside = bounds && drag.x >= bounds.left && drag.x <= bounds.right && drag.y >= bounds.top && drag.y <= bounds.bottom;
    const point = inside ? sceneRef.current?.resolvePhotoDropPoint(drag.x, drag.y) : null;
    const latest = latestPropsRef.current;
    if (drag.kind !== "photo" || !point || !latest.onResolvePhotoDropTarget) {
      if (drag.phase === "drop") latest.onPhotoDragCancel?.();
      return;
    }
    void latest.onResolvePhotoDropTarget(drag.mediaId, point).then((target) => {
      if (request !== dragRequestRef.current) return;
      if (drag.phase === "drop") {
        latestPropsRef.current.onPhotoDragCancel?.();
        if (target.kind !== "invalid") void latestPropsRef.current.onDropPhoto?.(drag.mediaId, point);
      } else if (target.kind !== "invalid") {
        setResolvedPhotoDrop({ request, mediaId: drag.mediaId, point, target });
      }
    }, () => {
      if (request === dragRequestRef.current && drag.phase === "drop") latestPropsRef.current.onPhotoDragCancel?.();
    });
    return () => { if (request === dragRequestRef.current) dragRequestRef.current += 1; };
  }, [props.mediaDrag, props.projectId]);

  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || !props.mediaDrag) return;
      dragRequestRef.current += 1;
      setResolvedPhotoDrop(null);
      decorative.clear();
      props.onPhotoDragCancel?.();
    };
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [props.mediaDrag, props.onPhotoDragCancel, decorative.clear]);

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

  // Materialize the committed Canvas before paint and before passive thumbnail
  // cleanup (e.g. an applied photo disappearing from the unused-media filter).
  useLayoutEffect(() => {
    const scene = sceneRef.current;
    const host = hostRef.current;
    if (!ready || !scene || !host) return;
    scene.update(sceneProps, host.clientHeight);
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

  function handleSheetContextMenu(event: ReactMouseEvent<HTMLDivElement>) {
    event.preventDefault();
    if (props.mode.kind !== "normal") return;
    const sheetId = sceneRef.current?.resolveSheetAtPoint(
      event.clientX,
      event.clientY,
    );
    if (!sheetId) return;
    props.onOpenSheetContextMenu?.(sheetId, {
      x: event.clientX,
      y: event.clientY,
    });
  }

  if (!hasSheets) {
    return (
      <EmptyState
        className="canvas-empty"
        description="Não há conteúdo de composição disponível neste Projeto."
        icon={<AppIcon icon={Layers3} size={18} />}
        title="Nenhuma Lâmina disponível"
      />
    );
  }

  return (
    <div className={`canvas-shell${isolated ? " canvas-shell--isolated" : ""}`}>
      <div
        className="canvas-host"
        data-media-drag={props.mediaDrag?.kind}
        data-centered-sheet-id={props.centeredSheetId ?? undefined}
        data-isolated-sheet-id={props.mode.kind === "normal" ? props.mode.isolatedSheetId : undefined}
        data-viewport-offset-x={props.viewport.offsetX}
        ref={hostRef}
        onContextMenu={handleSheetContextMenu}
      >
        {decorative.preview && props.mediaDrag && <span
          className="canvas-decorative-drop-label" role="status" aria-label="Aplicação do Decorativo"
          style={{ left: Math.max(8, Math.min((hostRef.current?.clientWidth ?? 0) - 210, props.mediaDrag.x - (hostRef.current?.getBoundingClientRect().left ?? 0) + 14)),
            top: Math.max(8, Math.min((hostRef.current?.clientHeight ?? 0) - 36, props.mediaDrag.y - (hostRef.current?.getBoundingClientRect().top ?? 0) + 14)) }}>
          {decorative.preview.role === "background" ? "Fundo" : "Overlay"} · {decorative.preview.scope === "bothSides" ? "Ambos os lados" : decorative.preview.scope === "left" ? "Lado esquerdo" : "Lado direito"}
        </span>}
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
        {props.mode.kind === "normal" && props.sheetReorder && props.mediaDrag?.kind !== "decorative" ? (
          <SheetBarOverlay
            bleedUm={props.technicalGuides?.bleedUm}
            disabled={props.sheetReorder.disabled || isolated}
            focusedSheetId={props.focusedSheetId}
            layout={barLayout}
            mediaPreviewUrls={props.mediaPreviewUrls}
            metrics={canvasMetrics}
            offsetY={barOffsetY}
            onAutoScrollVelocity={setSheetAutoScrollVelocity}
            onCancel={props.sheetReorder.onCancel}
            onContextMenu={(sheetId, position) =>
              props.onOpenSheetContextMenu?.(sheetId, position)
            }
            onDrop={props.sheetReorder.onDrop}
            onEditSheet={props.onEditSheet}
            onSelect={props.sheetReorder.onSelect}
            onSwapSides={props.sheetSideSwap?.onSwap}
            layouts={props.sheetLayouts}
            onBarHover={(sheetId, hovered, swapHovered) => sceneRef.current?.handleSheetBarHover(sheetId, hovered, swapHovered)}
            onSwapFocus={(sheetId, focused) => sceneRef.current?.handleSheetBarActionFocus(sheetId, "swap", focused)}
            onLayoutFocus={(sheetId, focused) => sceneRef.current?.handleSheetBarActionFocus(sheetId, "layout", focused)}
            onPreview={props.sheetReorder.onPreview}
            representation={isolated ? { ghost: null, placeholderIndex: null, order: visibleSheets.map((sheet) => sheet.sheetId) }
              : props.sheetReorder.representation}
            sheetBarMetadata={sheetBarMetadata}
            sheets={visibleSheets}
            status={props.sheetReorder.status}
            viewport={barViewport}
          />
        ) : null}
      </div>
      {!isolated && <CanvasHorizontalScrollbar
        centeredSheetId={props.centeredSheetId}
        layout={props.continuousCanvasLayout}
        metrics={canvasMetrics}
        mode={props.mode}
        onCenteredSheetChange={props.onCenteredSheetChange}
        onViewportChange={props.onViewportChange}
        viewport={props.viewport}
      />}
    </div>
  );
}
