import { useEffect, useRef, useState } from "react";
import {
  Application,
  Container,
  FederatedPointerEvent,
  FederatedWheelEvent,
  Graphics,
  Rectangle,
  Text,
} from "pixi.js";

import type {
  CompositionPlan,
  PhotoPlacement,
  Vector2,
} from "../domain/project";
import type { ViewportState } from "../state/editorView";
import {
  CANVAS_VERTICAL_MARGIN_PX,
  centeredSheetIdInContinuousCanvas,
  clampContinuousCanvasOffset,
  continuousCanvasScale,
  MICROMETER_TO_CANVAS_PIXEL,
  SHEET_LABEL_HEIGHT_PX,
  sheetOffsetInCanvasPixels,
} from "./canvasGeometry";
import {
  createPhotoGeometry,
  type PhotoGeometry,
} from "./photoGeometry";
import {
  advancePhotoZoomGesture,
  finishPhotoZoomGesture,
  type PhotoZoomGesture,
} from "./photoZoomGesture";
import {
  photoPaletteIndexForStripe,
  SHEET_VISUAL_STYLE,
} from "./sheetVisualStyle";

const PRELOAD_MARGIN = 1;
const ZOOM_GESTURE_SETTLE_MS = 500;
const PAN_OUTSIDE_OPACITY = 0.24;

interface PhotoZoomPreview {
  frameId: string;
  value: number;
}

export interface PhotoTransformPreview {
  frameId: string;
  panX: number;
  panY: number;
  zoom: number;
}

export interface CanvasMetrics {
  width: number;
  scale: number;
}

interface AlbumCanvasProps {
  composition: CompositionPlan;
  selectedFrameId: string | null;
  focusedSheetId: string;
  centeredSheetId: string;
  viewport: ViewportState;
  photoZoomPreview?: PhotoZoomPreview | null;
  onSelectFrame(frameId: string | null): void;
  onFocusSheet(sheetId: string): void;
  onCenteredSheetChange(sheetId: string): void;
  onViewportChange(viewport: ViewportState): void;
  onTransformPreview(preview: PhotoTransformPreview | null): void;
  onPanCommit(frameId: string, deltaX: number, deltaY: number): void;
  onZoomCommit(frameId: string, delta: number): void;
  onTransformCommit(
    frameId: string,
    deltaPanX: number,
    deltaPanY: number,
    deltaZoom: number,
  ): void;
  onMaterializedChange(count: number): void;
  onCanvasMetricsChange?(metrics: CanvasMetrics): void;
}

type CenteredSheetSynchronization = Pick<
  AlbumCanvasProps,
  "centeredSheetId" | "composition" | "onCenteredSheetChange"
>;

interface PhotoRenderNode {
  frameId: string;
  layer: Container;
  outsideLayer: Container;
  thirdsGuides: Graphics;
  geometry: PhotoGeometry;
  baseZoom: number;
  baseScaleX: number;
  originalX: number;
  originalY: number;
  pan: Vector2;
}

interface PhotoPreviewLayerOptions {
  label: string;
  drawWidth: number;
  drawHeight: number;
  center: Vector2;
  rotationDegrees: number;
  mirrorX: boolean;
  palette: readonly string[];
}

interface DragGesture {
  frameId: string;
  startX: number;
  startY: number;
  canvasScale: number;
  node: PhotoRenderNode;
  originalX: number;
  originalY: number;
  currentX: number;
  currentY: number;
  currentPan: Vector2;
  currentZoom: number;
}

interface ZoomGestureRuntime {
  gesture: PhotoZoomGesture;
  timer: number | null;
}

export function AlbumCanvas(props: AlbumCanvasProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const applicationRef = useRef<Application | null>(null);
  const dragRef = useRef<DragGesture | null>(null);
  const zoomGestureRef = useRef<ZoomGestureRuntime | null>(null);
  const photoNodesRef = useRef(new Map<string, PhotoRenderNode>());
  const externalPreviewFrameRef = useRef<string | null>(null);
  const lastCanvasMetricsRef = useRef<CanvasMetrics | null>(null);
  const propsRef = useRef(props);
  const [ready, setReady] = useState(false);
  const [canvasSizeRevision, setCanvasSizeRevision] = useState(0);
  propsRef.current = props;

  useEffect(() => {
    if (!hostRef.current || props.composition.sheets.length === 0) return;
    let disposed = false;
    let initialized = false;
    let destroyed = false;
    const app = new Application();
    const destroyInitializedApp = () => {
      if (!initialized || destroyed) return;
      destroyed = true;
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
        applicationRef.current = app;
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
      applicationRef.current = null;
      photoNodesRef.current.clear();
      if (zoomGestureRef.current) {
        if (zoomGestureRef.current.timer !== null) {
          window.clearTimeout(zoomGestureRef.current.timer);
        }
        zoomGestureRef.current = null;
      }
      destroyInitializedApp();
    };
  }, [props.composition.sheets.length]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host || typeof ResizeObserver === "undefined") return;

    const observer = new ResizeObserver(() => {
      applicationRef.current?.resize();
      setCanvasSizeRevision((revision) => revision + 1);
    });
    observer.observe(host);
    return () => observer.disconnect();
  }, [props.composition.sheets.length]);

  useEffect(() => {
    const app = applicationRef.current;
    if (!app || !ready) return;

    const removed = app.stage.removeChildren();
    removed.forEach((child) => child.destroy({ children: true }));
    photoNodesRef.current.clear();
    const world = new Container();
    app.stage.addChild(world);

    const sheetHeight =
      props.composition.sheets[0].heightUm *
      MICROMETER_TO_CANVAS_PIXEL;
    const canvasScale = continuousCanvasScale(
      hostRef.current?.clientHeight || app.screen.height,
      sheetHeight,
    );
    const boundedOffsetX = clampContinuousCanvasOffset(
      props.composition.sheets,
      props.viewport.offsetX,
      canvasScale,
      app.screen.width,
    );
    if (Math.abs(boundedOffsetX - props.viewport.offsetX) > 0.0001) {
      props.onViewportChange({
        ...props.viewport,
        offsetX: boundedOffsetX,
      });
    }
    synchronizeCenteredSheet(
      props,
      boundedOffsetX,
      canvasScale,
      app.screen.width,
    );
    const canvasMetrics = {
      width: app.screen.width,
      scale: canvasScale,
    };
    const previousCanvasMetrics = lastCanvasMetricsRef.current;
    if (
      previousCanvasMetrics === null ||
      Math.abs(previousCanvasMetrics.width - canvasMetrics.width) >
        0.0001 ||
      Math.abs(previousCanvasMetrics.scale - canvasMetrics.scale) >
        0.0001
    ) {
      lastCanvasMetricsRef.current = canvasMetrics;
      props.onCanvasMetricsChange?.(canvasMetrics);
    }

    const sheetOffsets = props.composition.sheets.map((_, index) =>
      sheetOffsetInCanvasPixels(props.composition.sheets, index),
    );

    const viewportLeft = -boundedOffsetX / canvasScale;
    const viewportRight = viewportLeft + app.screen.width / canvasScale;
    const visibleIndexes = props.composition.sheets
      .map((sheet, index) => ({
        index,
        left: sheetOffsets[index],
        right:
          sheetOffsets[index] +
          sheet.widthUm * MICROMETER_TO_CANVAS_PIXEL,
      }))
      .filter(
        ({ left, right }) =>
          right >= viewportLeft - 700 && left <= viewportRight + 700,
      )
      .map(({ index }) => index);
    const firstVisible = Math.max(
      0,
      (visibleIndexes[0] ?? 0) - PRELOAD_MARGIN,
    );
    const lastVisible = Math.min(
      props.composition.sheets.length - 1,
      (visibleIndexes[visibleIndexes.length - 1] ?? 0) + PRELOAD_MARGIN,
    );
    const materialized = Math.max(0, lastVisible - firstVisible + 1);
    props.onMaterializedChange(materialized);

    world.position.set(
      boundedOffsetX,
      CANVAS_VERTICAL_MARGIN_PX +
        SHEET_LABEL_HEIGHT_PX * canvasScale,
    );
    world.scale.set(canvasScale);

    for (let index = firstVisible; index <= lastVisible; index += 1) {
      const sheet = props.composition.sheets[index];
      const sheetContainer = new Container();
      const width =
        sheet.widthUm * MICROMETER_TO_CANVAS_PIXEL;
      const height =
        sheet.heightUm * MICROMETER_TO_CANVAS_PIXEL;
      sheetContainer.position.set(sheetOffsets[index], 0);
      sheetContainer.eventMode = "static";
      sheetContainer.hitArea = new Rectangle(0, 0, width, height);
      sheetContainer.cursor = "default";
      sheetContainer.on("pointertap", (event: FederatedPointerEvent) => {
        if (event.target === sheetContainer) {
          propsRef.current.onSelectFrame(null);
          propsRef.current.onFocusSheet(sheet.sheetId);
        }
      });

      const shadow = new Graphics()
        .roundRect(8, 12, width, height, 4)
        .fill({ color: 0x121820, alpha: 0.2 });
      const surface = new Graphics()
        .roundRect(
          0,
          0,
          width,
          height,
          SHEET_VISUAL_STYLE.surface.cornerRadiusPx,
        )
        .fill({
          color: hexToNumber(SHEET_VISUAL_STYLE.surface.fill),
        })
        .stroke({
          color: hexToNumber(SHEET_VISUAL_STYLE.surface.outline),
          width: SHEET_VISUAL_STYLE.surface.outlineWidthPx,
          alpha: SHEET_VISUAL_STYLE.surface.outlineOpacity,
        });
      sheetContainer.addChild(shadow, surface);

      const label = new Text({
        text: `LÂMINA ${String(sheet.number).padStart(2, "0")}`,
        style: {
          fontFamily: "Segoe UI",
          fontSize: 10,
          fontWeight: "600",
          fill: 0x77808a,
          letterSpacing: 1.4,
        },
      });
      label.position.set(2, -SHEET_LABEL_HEIGHT_PX);
      sheetContainer.addChild(label);

      const centerLine = new Graphics()
        .moveTo(width / 2, 0)
        .lineTo(width / 2, height)
        .stroke({
          color: hexToNumber(SHEET_VISUAL_STYLE.centerLine.color),
          width: SHEET_VISUAL_STYLE.centerLine.widthPx,
          alpha: SHEET_VISUAL_STYLE.centerLine.opacity,
        });
      sheetContainer.addChild(centerLine);

      for (const frame of sheet.frames) {
        const frameContainer = new Container();
        const frameX =
          frame.clipRect.x * MICROMETER_TO_CANVAS_PIXEL;
        const frameY =
          frame.clipRect.y * MICROMETER_TO_CANVAS_PIXEL;
        const frameWidth =
          frame.clipRect.width * MICROMETER_TO_CANVAS_PIXEL;
        const frameHeight =
          frame.clipRect.height * MICROMETER_TO_CANVAS_PIXEL;
        frameContainer.position.set(frameX, frameY);
        frameContainer.eventMode = "static";
        frameContainer.hitArea = new Rectangle(
          0,
          0,
          frameWidth,
          frameHeight,
        );
        frameContainer.cursor = frame.photo ? "grab" : "pointer";

        let photoNode: PhotoRenderNode | null = null;
        if (frame.photo) {
          const geometry = createPhotoGeometry(
            frame.photo.placement,
            MICROMETER_TO_CANVAS_PIXEL,
          );
          const drawWidth = geometry.current.size.width;
          const drawHeight = geometry.current.size.height;
          const previewOptions = {
            drawWidth,
            drawHeight,
            center: geometry.current.center,
            rotationDegrees: frame.photo.rotationDegrees,
            mirrorX: frame.photo.mirrorX,
            palette: frame.photo.palette,
          };
          const outsidePhotoLayer = createPhotoPreviewLayer({
            ...previewOptions,
            label: "photo-pan-outside-preview",
          });
          outsidePhotoLayer.alpha = PAN_OUTSIDE_OPACITY;
          outsidePhotoLayer.eventMode = "none";
          outsidePhotoLayer.visible = false;
          const photoLayer = createPhotoPreviewLayer({
            ...previewOptions,
            label: "photo-pan-inside-preview",
          });

          const clip = new Graphics()
            .rect(0, 0, frameWidth, frameHeight)
            .fill(0xffffff);
          const photoViewport = new Container();
          photoViewport.addChild(photoLayer);
          photoViewport.mask = clip;
          const thirdsGuides = createThirdsGuides(
            frameWidth,
            frameHeight,
          );
          frameContainer.addChild(
            outsidePhotoLayer,
            photoViewport,
            clip,
            thirdsGuides,
          );

          const baseZoom = frame.photo.placement.currentZoom;
          photoNode = {
            frameId: frame.frameId,
            layer: photoLayer,
            outsideLayer: outsidePhotoLayer,
            thirdsGuides,
            geometry,
            baseZoom,
            baseScaleX: frame.photo.mirrorX ? -1 : 1,
            originalX: photoLayer.x,
            originalY: photoLayer.y,
            pan: frame.photo.placement.currentPan,
          };
          photoNodesRef.current.set(frame.frameId, photoNode);
          if (props.photoZoomPreview?.frameId === frame.frameId) {
            applyPhotoZoomPreview(
              photoNode,
              props.photoZoomPreview.value,
            );
          }
        } else {
          const placeholder = new Graphics()
            .rect(0, 0, frameWidth, frameHeight)
            .fill({
              color: hexToNumber(
                SHEET_VISUAL_STYLE.placeholder.fill,
              ),
            })
            .stroke({
              color: hexToNumber(
                SHEET_VISUAL_STYLE.placeholder.outline,
              ),
              width:
                SHEET_VISUAL_STYLE.placeholder.outlineWidthPx,
              alpha:
                SHEET_VISUAL_STYLE.placeholder.outlineOpacity,
            });
          const cross = new Graphics()
            .moveTo(
              frameWidth / 2 -
                SHEET_VISUAL_STYLE.placeholder.crossHalfLengthPx,
              frameHeight / 2,
            )
            .lineTo(
              frameWidth / 2 +
                SHEET_VISUAL_STYLE.placeholder.crossHalfLengthPx,
              frameHeight / 2,
            )
            .moveTo(
              frameWidth / 2,
              frameHeight / 2 -
                SHEET_VISUAL_STYLE.placeholder.crossHalfLengthPx,
            )
            .lineTo(
              frameWidth / 2,
              frameHeight / 2 +
                SHEET_VISUAL_STYLE.placeholder.crossHalfLengthPx,
            )
            .stroke({
              color: hexToNumber(
                SHEET_VISUAL_STYLE.placeholder.crossColor,
              ),
              width: SHEET_VISUAL_STYLE.placeholder.crossWidthPx,
              alpha: SHEET_VISUAL_STYLE.placeholder.crossOpacity,
            });
          frameContainer.addChild(placeholder, cross);
        }

        const outline = new Graphics()
          .rect(0, 0, frameWidth, frameHeight)
          .stroke({
            color:
              props.selectedFrameId === frame.frameId
                ? 0xb8874f
                : hexToNumber(SHEET_VISUAL_STYLE.frame.outline),
            width:
              props.selectedFrameId === frame.frameId
                ? 3
                : SHEET_VISUAL_STYLE.frame.outlineWidthPx,
            alpha:
              props.selectedFrameId === frame.frameId
                ? 1
                : SHEET_VISUAL_STYLE.frame.outlineOpacity,
          });
        frameContainer.addChild(outline);

        frameContainer.on("pointertap", (event: FederatedPointerEvent) => {
          event.stopPropagation();
          if (!event.altKey) {
            propsRef.current.onSelectFrame(frame.frameId);
            propsRef.current.onFocusSheet(sheet.sheetId);
          }
        });
        frameContainer.on("pointerdown", (event: FederatedPointerEvent) => {
          if (!event.altKey || !photoNode) return;
          event.stopPropagation();
          const activeZoom = zoomGestureRef.current;
          const continuesZoom =
            activeZoom?.gesture.frameId === frame.frameId;
          if (continuesZoom && activeZoom.timer !== null) {
            window.clearTimeout(activeZoom.timer);
            activeZoom.timer = null;
          }
          dragRef.current = {
            frameId: frame.frameId,
            startX: event.global.x,
            startY: event.global.y,
            canvasScale,
            node: photoNode,
            originalX: photoNode.layer.x,
            originalY: photoNode.layer.y,
            currentX: photoNode.layer.x,
            currentY: photoNode.layer.y,
            currentPan: photoNode.pan,
            currentZoom: continuesZoom
              ? activeZoom.gesture.baseZoom +
                activeZoom.gesture.delta
              : photoNode.baseZoom,
          };
          setPhotoPanAids(photoNode, true);
          frameContainer.cursor = "grabbing";
        });
        frameContainer.on("wheel", (event: FederatedWheelEvent) => {
          if (!event.altKey || !photoNode) return;
          event.preventDefault();

          const current = zoomGestureRef.current;
          if (current?.timer != null) {
            window.clearTimeout(current.timer);
          }
          const transition = advancePhotoZoomGesture(
            current?.gesture ?? null,
            {
              frameId: frame.frameId,
              baseZoom: photoNode.baseZoom,
              zoomRange: photoNode.geometry.zoomRange,
              wheelDeltaY: event.deltaY,
            },
          );
          if (transition.interruptedCommit) {
            propsRef.current.onZoomCommit(
              transition.interruptedCommit.frameId,
              transition.interruptedCommit.delta,
            );
          }

          const activeDrag = dragRef.current;
          if (activeDrag?.frameId === frame.frameId) {
            const combined = photoNode.geometry.constrain(
              {
                x: activeDrag.currentX,
                y: activeDrag.currentY,
              },
              transition.previewZoom,
            );
            activeDrag.currentX = combined.placement.center.x;
            activeDrag.currentY = combined.placement.center.y;
            activeDrag.currentPan = combined.pan;
            activeDrag.currentZoom = combined.zoom;
            applyPhotoPlacementPreview(
              photoNode,
              combined.zoom,
              combined.placement,
            );
            propsRef.current.onTransformPreview(
              createTransformPreview(
                activeDrag.frameId,
                activeDrag.currentPan,
                activeDrag.currentZoom,
              ),
            );
            zoomGestureRef.current = {
              gesture: transition.gesture,
              timer: null,
            };
            return;
          }

          applyPhotoZoomPreview(photoNode, transition.previewZoom);
          propsRef.current.onTransformPreview(
            createTransformPreview(
              photoNode.frameId,
              photoNode.pan,
              transition.previewZoom,
            ),
          );
          const timer = window.setTimeout(() => {
            const runtime = zoomGestureRef.current;
            const commit = finishPhotoZoomGesture(
              runtime?.gesture ?? null,
            );
            if (commit) {
              propsRef.current.onZoomCommit(
                commit.frameId,
                commit.delta,
              );
            } else {
              propsRef.current.onTransformPreview(null);
            }
            zoomGestureRef.current = null;
          }, ZOOM_GESTURE_SETTLE_MS);
          zoomGestureRef.current = {
            gesture: transition.gesture,
            timer,
          };
        });
        sheetContainer.addChild(frameContainer);
      }

      if (sheet.hasOverlay) {
        const overlayStyle = SHEET_VISUAL_STYLE.overlay;
        const overlay = new Graphics()
          .roundRect(
            overlayStyle.insetPx,
            overlayStyle.insetPx,
            width - overlayStyle.insetPx * 2,
            height - overlayStyle.insetPx * 2,
            overlayStyle.cornerRadiusPx,
          )
          .stroke({
            color: hexToNumber(overlayStyle.outline),
            width: overlayStyle.outlineWidthPx,
            alpha: overlayStyle.outlineOpacity,
          });
        overlay.eventMode = "none";
        sheetContainer.addChild(overlay);
      }

      if (sheet.sheetId === props.focusedSheetId) {
        const focus = new Graphics()
          .roundRect(-5, -5, width + 10, height + 10, 7)
          .stroke({ color: 0xc99a5d, width: 2, alpha: 0.9 });
        focus.eventMode = "none";
        sheetContainer.addChild(focus);
      }
      world.addChild(sheetContainer);
    }

    app.stage.eventMode = "static";
    app.stage.hitArea = new Rectangle(
      0,
      0,
      app.screen.width,
      app.screen.height,
    );
    app.stage.on("globalpointermove", (event: FederatedPointerEvent) => {
      const gesture = dragRef.current;
      if (!gesture) return;
      updatePanPreview(gesture, event.global.x, event.global.y);
      propsRef.current.onTransformPreview(
        createTransformPreview(
          gesture.frameId,
          gesture.currentPan,
          gesture.currentZoom,
        ),
      );
    });
    const finishDrag = (event: FederatedPointerEvent) => {
      const gesture = dragRef.current;
      if (!gesture) return;
      updatePanPreview(gesture, event.global.x, event.global.y);
      dragRef.current = null;
      setPhotoPanAids(gesture.node, false);

      const deltaX = gesture.currentPan.x - gesture.node.pan.x;
      const deltaY = gesture.currentPan.y - gesture.node.pan.y;
      const deltaZoom =
        gesture.currentZoom - gesture.node.baseZoom;
      const combinedZoom = zoomGestureRef.current;
      const ownsCombinedZoom =
        combinedZoom?.gesture.frameId === gesture.frameId;
      const changedPan =
        Math.abs(deltaX) > 0.0001 ||
        Math.abs(deltaY) > 0.0001;
      const changedZoom = Math.abs(deltaZoom) > 0.0001;

      if (ownsCombinedZoom) {
        if (combinedZoom.timer !== null) {
          window.clearTimeout(combinedZoom.timer);
        }
        zoomGestureRef.current = null;
      }

      if ((ownsCombinedZoom && changedZoom) || changedPan) {
        propsRef.current.onTransformPreview(
          createTransformPreview(
            gesture.frameId,
            gesture.currentPan,
            gesture.currentZoom,
          ),
        );
      } else {
        propsRef.current.onTransformPreview(null);
      }

      if (ownsCombinedZoom && changedZoom) {
        propsRef.current.onTransformCommit(
          gesture.frameId,
          deltaX,
          deltaY,
          deltaZoom,
        );
      } else if (changedPan) {
        propsRef.current.onPanCommit(
          gesture.frameId,
          deltaX,
          deltaY,
        );
      }
    };
    app.stage.on("pointerup", finishDrag);
    app.stage.on("pointerupoutside", finishDrag);
    app.stage.on("pointercancel", () => {
      const gesture = dragRef.current;
      if (!gesture) return;
      dragRef.current = null;
      setPhotoPanAids(gesture.node, false);
      resetPhotoPreview(gesture.node);
      propsRef.current.onTransformPreview(null);

      const combinedZoom = zoomGestureRef.current;
      if (combinedZoom?.gesture.frameId === gesture.frameId) {
        if (combinedZoom.timer !== null) {
          window.clearTimeout(combinedZoom.timer);
        }
        zoomGestureRef.current = null;
      }
    });

    const handleWheel = (event: WheelEvent) => {
      if (event.altKey) return;
      event.preventDefault();
      if (event.ctrlKey) return;
      const nextOffset = clampContinuousCanvasOffset(
        props.composition.sheets,
        propsRef.current.viewport.offsetX -
          (event.deltaX || event.deltaY) * 0.9,
        canvasScale,
        app.screen.width,
      );
      propsRef.current.onViewportChange({
        ...propsRef.current.viewport,
        offsetX: nextOffset,
      });
      synchronizeCenteredSheet(
        propsRef.current,
        nextOffset,
        canvasScale,
        app.screen.width,
      );
    };
    app.canvas.addEventListener("wheel", handleWheel, { passive: false });

    return () => {
      app.canvas.removeEventListener("wheel", handleWheel);
      app.stage.removeAllListeners();
      photoNodesRef.current.clear();
    };
  }, [
    props.composition,
    props.centeredSheetId,
    props.focusedSheetId,
    props.onMaterializedChange,
    props.selectedFrameId,
    props.viewport.offsetX,
    canvasSizeRevision,
    ready,
  ]);

  useEffect(() => {
    const previousFrameId = externalPreviewFrameRef.current;
    const nextPreview = props.photoZoomPreview;

    if (previousFrameId && previousFrameId !== nextPreview?.frameId) {
      const previousNode = photoNodesRef.current.get(previousFrameId);
      if (previousNode) resetPhotoPreview(previousNode);
    }

    if (nextPreview) {
      const nextNode = photoNodesRef.current.get(nextPreview.frameId);
      if (nextNode) applyPhotoZoomPreview(nextNode, nextPreview.value);
      externalPreviewFrameRef.current = nextPreview.frameId;
    } else {
      if (previousFrameId) {
        const previousNode = photoNodesRef.current.get(previousFrameId);
        if (previousNode) resetPhotoPreview(previousNode);
      }
      externalPreviewFrameRef.current = null;
    }
  }, [props.photoZoomPreview]);

  if (props.composition.sheets.length === 0) {
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

function synchronizeCenteredSheet(
  props: CenteredSheetSynchronization,
  offsetX: number,
  scale: number,
  canvasWidth: number,
) {
  const centeredSheetId = centeredSheetIdInContinuousCanvas(
    props.composition.sheets,
    offsetX,
    scale,
    canvasWidth,
  );
  if (
    centeredSheetId &&
    centeredSheetId !== props.centeredSheetId
  ) {
    props.onCenteredSheetChange(centeredSheetId);
  }
}

function createPhotoPreviewLayer({
  label,
  drawWidth,
  drawHeight,
  center,
  rotationDegrees,
  mirrorX,
  palette,
}: PhotoPreviewLayerOptions) {
  const photoLayer = new Container();
  photoLayer.label = label;
  photoLayer.pivot.set(drawWidth / 2, drawHeight / 2);
  photoLayer.position.set(center.x, center.y);
  photoLayer.rotation = (rotationDegrees * Math.PI) / 180;
  photoLayer.scale.set(mirrorX ? -1 : 1, 1);

  const photoStyle = SHEET_VISUAL_STYLE.photo;
  const stripeCount = photoStyle.stripeCount;
  for (let stripe = 0; stripe < stripeCount; stripe += 1) {
    const paletteIndex = photoPaletteIndexForStripe(stripe);
    const stripeGraphic = new Graphics()
      .rect(
        (drawWidth / stripeCount) * stripe,
        0,
        drawWidth / stripeCount + photoStyle.stripeOverlapPx,
        drawHeight,
      )
      .fill({
        color: hexToNumber(palette[paletteIndex]),
      });
    photoLayer.addChild(stripeGraphic);
  }
  const light = new Graphics()
    .circle(
      drawWidth * photoStyle.lightCenterXRatio,
      drawHeight * photoStyle.lightCenterYRatio,
      drawHeight * photoStyle.lightRadiusToHeightRatio,
    )
    .fill({
      color: hexToNumber(photoStyle.lightColor),
      alpha: photoStyle.lightOpacity,
    });
  photoLayer.addChild(light);

  return photoLayer;
}

function createTransformPreview(
  frameId: string,
  pan: Vector2,
  zoom: number,
): PhotoTransformPreview {
  return {
    frameId,
    panX: pan.x,
    panY: pan.y,
    zoom,
  };
}

function createThirdsGuides(frameWidth: number, frameHeight: number) {
  const guides = new Graphics()
    .moveTo(frameWidth / 3, 0)
    .lineTo(frameWidth / 3, frameHeight)
    .moveTo((frameWidth * 2) / 3, 0)
    .lineTo((frameWidth * 2) / 3, frameHeight)
    .moveTo(0, frameHeight / 3)
    .lineTo(frameWidth, frameHeight / 3)
    .moveTo(0, (frameHeight * 2) / 3)
    .lineTo(frameWidth, (frameHeight * 2) / 3)
    .stroke({ color: 0xffffff, width: 1.2, alpha: 0.88 });
  guides.label = "photo-pan-thirds-guides";
  guides.eventMode = "none";
  guides.visible = false;
  return guides;
}

function setPhotoPanAids(node: PhotoRenderNode, visible: boolean) {
  node.outsideLayer.visible = visible;
  node.thirdsGuides.visible = visible;
}

function updatePanPreview(
  gesture: DragGesture,
  pointerX: number,
  pointerY: number,
) {
  const nextX =
    gesture.originalX +
    (pointerX - gesture.startX) / gesture.canvasScale;
  const nextY =
    gesture.originalY +
    (pointerY - gesture.startY) / gesture.canvasScale;
  const constrained = gesture.node.geometry.constrain(
    { x: nextX, y: nextY },
    gesture.currentZoom,
  );
  gesture.currentX = constrained.placement.center.x;
  gesture.currentY = constrained.placement.center.y;
  gesture.currentPan = constrained.pan;
  setPhotoLayersPosition(
    gesture.node,
    gesture.currentX,
    gesture.currentY,
  );
}

function applyPhotoZoomPreview(node: PhotoRenderNode, targetZoom: number) {
  const zoomed = node.geometry.zoom(targetZoom);
  applyPhotoPlacementPreview(node, zoomed.zoom, zoomed.placement);
}

function applyPhotoPlacementPreview(
  node: PhotoRenderNode,
  targetZoom: number,
  placement: PhotoPlacement,
) {
  const factor = targetZoom / node.baseZoom;
  setPhotoLayersScale(node, node.baseScaleX * factor, factor);
  setPhotoLayersPosition(
    node,
    placement.center.x,
    placement.center.y,
  );
}

function resetPhotoPreview(node: PhotoRenderNode) {
  setPhotoLayersScale(node, node.baseScaleX, 1);
  setPhotoLayersPosition(node, node.originalX, node.originalY);
}

function setPhotoLayersPosition(
  node: PhotoRenderNode,
  x: number,
  y: number,
) {
  node.layer.position.set(x, y);
  node.outsideLayer.position.set(x, y);
}

function setPhotoLayersScale(
  node: PhotoRenderNode,
  x: number,
  y: number,
) {
  node.layer.scale.set(x, y);
  node.outsideLayer.scale.set(x, y);
}

function hexToNumber(value: string): number {
  return Number.parseInt(value.replace("#", ""), 16);
}
