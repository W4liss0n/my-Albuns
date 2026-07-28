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

import type { CompositionPlan } from "../domain/project";
import type { ViewportState } from "../state/editorView";
import {
  CANVAS_VERTICAL_MARGIN_PX,
  continuousCanvasScale,
  MICROMETER_TO_CANVAS_PIXEL,
  SHEET_LABEL_HEIGHT_PX,
  sheetOffsetInCanvasPixels,
} from "./canvasGeometry";

const PRELOAD_MARGIN = 1;
const ZOOM_GESTURE_SETTLE_MS = 500;

interface PhotoZoomPreview {
  frameId: string;
  value: number;
}

interface AlbumCanvasProps {
  composition: CompositionPlan;
  selectedFrameId: string | null;
  focusedSheetId: string;
  viewport: ViewportState;
  photoZoomByFrameId?: Readonly<Record<string, number>>;
  photoZoomPreview?: PhotoZoomPreview | null;
  onSelectFrame(frameId: string | null): void;
  onFocusSheet(sheetId: string): void;
  onViewportChange(viewport: ViewportState): void;
  onPanCommit(frameId: string, deltaX: number, deltaY: number): void;
  onZoomCommit(frameId: string, delta: number): void;
  onMaterializedChange(count: number): void;
  onAutoScaleChange?(scale: number): void;
}

interface PhotoRenderNode {
  frameId: string;
  layer: Container;
  baseZoom: number;
  baseScaleX: number;
  drawWidth: number;
  drawHeight: number;
  frameWidth: number;
  frameHeight: number;
  rotationCosine: number;
  rotationSine: number;
  originalX: number;
  originalY: number;
  panX: number;
  panY: number;
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
}

interface ZoomGesture {
  frameId: string;
  baseZoom: number;
  delta: number;
  timer: number;
}

export function AlbumCanvas(props: AlbumCanvasProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const applicationRef = useRef<Application | null>(null);
  const dragRef = useRef<DragGesture | null>(null);
  const zoomGestureRef = useRef<ZoomGesture | null>(null);
  const photoNodesRef = useRef(new Map<string, PhotoRenderNode>());
  const externalPreviewFrameRef = useRef<string | null>(null);
  const lastAutoScaleRef = useRef<number | null>(null);
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
        window.clearTimeout(zoomGestureRef.current.timer);
        zoomGestureRef.current = null;
      }
      destroyInitializedApp();
    };
  }, [props.composition.sheets.length]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host || typeof ResizeObserver === "undefined") return;

    const observer = new ResizeObserver(() => {
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
    if (
      lastAutoScaleRef.current === null ||
      Math.abs(lastAutoScaleRef.current - canvasScale) > 0.0001
    ) {
      lastAutoScaleRef.current = canvasScale;
      props.onAutoScaleChange?.(canvasScale);
    }

    const sheetOffsets = props.composition.sheets.map((_, index) =>
      sheetOffsetInCanvasPixels(props.composition.sheets, index),
    );

    const viewportLeft = -props.viewport.offsetX / canvasScale;
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
      props.viewport.offsetX,
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
        .roundRect(0, 0, width, height, 3)
        .fill({ color: 0xf1ece2 })
        .stroke({ color: 0xffffff, width: 1, alpha: 0.65 });
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
        .stroke({ color: 0x887b6c, width: 1, alpha: 0.32 });
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
          const drawWidth =
            frame.photo.drawRect.width * MICROMETER_TO_CANVAS_PIXEL;
          const drawHeight =
            frame.photo.drawRect.height * MICROMETER_TO_CANVAS_PIXEL;
          const photoLayer = new Container();
          photoLayer.pivot.set(drawWidth / 2, drawHeight / 2);
          photoLayer.position.set(
            (frame.photo.drawRect.x - frame.clipRect.x) *
              MICROMETER_TO_CANVAS_PIXEL +
              drawWidth / 2,
            (frame.photo.drawRect.y - frame.clipRect.y) *
              MICROMETER_TO_CANVAS_PIXEL +
              drawHeight / 2,
          );
          photoLayer.rotation =
            (frame.photo.rotationDegrees * Math.PI) / 180;
          photoLayer.scale.set(frame.photo.mirrorX ? -1 : 1, 1);

          const stripeCount = 12;
          for (let stripe = 0; stripe < stripeCount; stripe += 1) {
            const paletteIndex = Math.min(
              2,
              Math.floor((stripe / stripeCount) * 3),
            );
            const stripeGraphic = new Graphics()
              .rect(
                (drawWidth / stripeCount) * stripe,
                0,
                drawWidth / stripeCount + 1,
                drawHeight,
              )
              .fill({
                color: hexToNumber(frame.photo.palette[paletteIndex]),
              });
            photoLayer.addChild(stripeGraphic);
          }
          const light = new Graphics()
            .circle(
              drawWidth * 0.73,
              drawHeight * 0.28,
              drawHeight * 0.18,
            )
            .fill({ color: 0xfff3d0, alpha: 0.32 });
          photoLayer.addChild(light);

          const clip = new Graphics()
            .rect(0, 0, frameWidth, frameHeight)
            .fill(0xffffff);
          const photoViewport = new Container();
          photoViewport.addChild(photoLayer);
          photoViewport.mask = clip;
          frameContainer.addChild(photoViewport, clip);

          const rotationCosine = Math.cos(photoLayer.rotation);
          const rotationSine = Math.sin(photoLayer.rotation);
          const baseZoom =
            props.photoZoomByFrameId?.[frame.frameId] ?? 1;
          photoNode = {
            frameId: frame.frameId,
            layer: photoLayer,
            baseZoom,
            baseScaleX: frame.photo.mirrorX ? -1 : 1,
            drawWidth,
            drawHeight,
            frameWidth,
            frameHeight,
            rotationCosine,
            rotationSine,
            originalX: photoLayer.x,
            originalY: photoLayer.y,
            panX: 0,
            panY: 0,
          };
          const normalizedPan = normalizedPhotoPan(
            photoNode,
            photoLayer.x,
            photoLayer.y,
            1,
          );
          photoNode.panX = normalizedPan.x;
          photoNode.panY = normalizedPan.y;
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
            .fill({ color: 0xded8cc })
            .stroke({ color: 0xb9b1a4, width: 1, alpha: 0.8 });
          const cross = new Graphics()
            .moveTo(frameWidth / 2 - 12, frameHeight / 2)
            .lineTo(frameWidth / 2 + 12, frameHeight / 2)
            .moveTo(frameWidth / 2, frameHeight / 2 - 12)
            .lineTo(frameWidth / 2, frameHeight / 2 + 12)
            .stroke({ color: 0x948b7e, width: 1.4, alpha: 0.75 });
          frameContainer.addChild(placeholder, cross);
        }

        const outline = new Graphics()
          .rect(0, 0, frameWidth, frameHeight)
          .stroke({
            color:
              props.selectedFrameId === frame.frameId ? 0xb8874f : 0xffffff,
            width: props.selectedFrameId === frame.frameId ? 3 : 1,
            alpha: props.selectedFrameId === frame.frameId ? 1 : 0.72,
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
          };
          frameContainer.cursor = "grabbing";
        });
        frameContainer.on("wheel", (event: FederatedWheelEvent) => {
          if (!event.altKey || !photoNode) return;
          event.preventDefault();

          const current = zoomGestureRef.current;
          if (current && current.frameId !== frame.frameId) {
            window.clearTimeout(current.timer);
            if (Math.abs(current.delta) > 0.0001) {
              propsRef.current.onZoomCommit(
                current.frameId,
                current.delta,
              );
            }
          } else if (current) {
            window.clearTimeout(current.timer);
          }

          const baseZoom =
            current?.frameId === frame.frameId
              ? current.baseZoom
              : (propsRef.current.photoZoomByFrameId?.[frame.frameId] ?? 1);
          const previousDelta =
            current?.frameId === frame.frameId ? current.delta : 0;
          const eventDelta = clamp(-event.deltaY * 0.0012, -0.18, 0.18);
          const accumulated = clamp(
            previousDelta + eventDelta,
            1 - baseZoom,
            4 - baseZoom,
          );

          applyPhotoZoomPreview(photoNode, baseZoom + accumulated);
          const timer = window.setTimeout(() => {
            const gesture = zoomGestureRef.current;
            if (!gesture) return;
            if (Math.abs(gesture.delta) > 0.0001) {
              propsRef.current.onZoomCommit(
                gesture.frameId,
                gesture.delta,
              );
            }
            zoomGestureRef.current = null;
          }, ZOOM_GESTURE_SETTLE_MS);
          zoomGestureRef.current = {
            frameId: frame.frameId,
            baseZoom,
            delta: accumulated,
            timer,
          };
        });
        sheetContainer.addChild(frameContainer);
      }

      if (sheet.hasOverlay) {
        const overlay = new Graphics()
          .roundRect(8, 8, width - 16, height - 16, 2)
          .stroke({ color: 0xd4b279, width: 2, alpha: 0.45 });
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
    });
    const finishDrag = (event: FederatedPointerEvent) => {
      const gesture = dragRef.current;
      if (!gesture) return;
      updatePanPreview(gesture, event.global.x, event.global.y);
      dragRef.current = null;

      const currentPan = normalizedPhotoPan(
        gesture.node,
        gesture.currentX,
        gesture.currentY,
        1,
      );
      const deltaX = currentPan.x - gesture.node.panX;
      const deltaY = currentPan.y - gesture.node.panY;

      if (Math.abs(deltaX) > 0.0001 || Math.abs(deltaY) > 0.0001) {
        propsRef.current.onPanCommit(
          gesture.frameId,
          clamp(deltaX, -2, 2),
          clamp(deltaY, -2, 2),
        );
      }
    };
    app.stage.on("pointerup", finishDrag);
    app.stage.on("pointerupoutside", finishDrag);

    const handleWheel = (event: WheelEvent) => {
      if (event.altKey) return;
      event.preventDefault();
      if (event.ctrlKey) return;
      propsRef.current.onViewportChange({
        ...propsRef.current.viewport,
        offsetX:
          propsRef.current.viewport.offsetX -
          (event.deltaX || event.deltaY) * 0.9,
      });
    };
    app.canvas.addEventListener("wheel", handleWheel, { passive: false });

    return () => {
      app.canvas.removeEventListener("wheel", handleWheel);
      app.stage.removeAllListeners();
      photoNodesRef.current.clear();
    };
  }, [
    props.composition,
    props.focusedSheetId,
    props.onMaterializedChange,
    props.photoZoomByFrameId,
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
  const clampedCenter = clampPhotoCenter(
    gesture.node,
    nextX,
    nextY,
    1,
  );
  gesture.currentX = clampedCenter.x;
  gesture.currentY = clampedCenter.y;
  gesture.node.layer.position.set(gesture.currentX, gesture.currentY);
}

function applyPhotoZoomPreview(node: PhotoRenderNode, targetZoom: number) {
  const boundedZoom = clamp(targetZoom, 1, 4);
  const factor = boundedZoom / node.baseZoom;
  node.layer.scale.set(node.baseScaleX * factor, factor);
  const targetCenter = photoCenterForPan(
    node,
    node.panX,
    node.panY,
    factor,
  );
  node.layer.position.set(targetCenter.x, targetCenter.y);
}

function resetPhotoPreview(node: PhotoRenderNode) {
  node.layer.scale.set(node.baseScaleX, 1);
  node.layer.position.set(node.originalX, node.originalY);
}

function photoPanRanges(node: PhotoRenderNode, scale: number) {
  let minimumFrameU = Number.POSITIVE_INFINITY;
  let maximumFrameU = Number.NEGATIVE_INFINITY;
  let minimumFrameV = Number.POSITIVE_INFINITY;
  let maximumFrameV = Number.NEGATIVE_INFINITY;

  for (const [cornerX, cornerY] of [
    [0, 0],
    [node.frameWidth, 0],
    [0, node.frameHeight],
    [node.frameWidth, node.frameHeight],
  ]) {
    const u =
      node.rotationCosine * cornerX + node.rotationSine * cornerY;
    const v =
      -node.rotationSine * cornerX + node.rotationCosine * cornerY;
    minimumFrameU = Math.min(minimumFrameU, u);
    maximumFrameU = Math.max(maximumFrameU, u);
    minimumFrameV = Math.min(minimumFrameV, v);
    maximumFrameV = Math.max(maximumFrameV, v);
  }

  const halfWidth = (node.drawWidth * scale) / 2;
  const halfHeight = (node.drawHeight * scale) / 2;
  return {
    minimumU: maximumFrameU - halfWidth,
    maximumU: minimumFrameU + halfWidth,
    minimumV: maximumFrameV - halfHeight,
    maximumV: minimumFrameV + halfHeight,
  };
}

function clampPhotoCenter(
  node: PhotoRenderNode,
  centerX: number,
  centerY: number,
  scale: number,
) {
  const ranges = photoPanRanges(node, scale);
  const centerU =
    node.rotationCosine * centerX + node.rotationSine * centerY;
  const centerV =
    -node.rotationSine * centerX + node.rotationCosine * centerY;
  const clampedU = clamp(centerU, ranges.minimumU, ranges.maximumU);
  const clampedV = clamp(centerV, ranges.minimumV, ranges.maximumV);

  return {
    x:
      node.rotationCosine * clampedU -
      node.rotationSine * clampedV,
    y:
      node.rotationSine * clampedU +
      node.rotationCosine * clampedV,
  };
}

function normalizedPhotoPan(
  node: PhotoRenderNode,
  centerX: number,
  centerY: number,
  scale: number,
) {
  const ranges = photoPanRanges(node, scale);
  const centerU =
    node.rotationCosine * centerX + node.rotationSine * centerY;
  const centerV =
    -node.rotationSine * centerX + node.rotationCosine * centerY;

  return {
    x: normalizedPosition(
      centerU,
      ranges.minimumU,
      ranges.maximumU,
    ),
    y: normalizedPosition(
      centerV,
      ranges.minimumV,
      ranges.maximumV,
    ),
  };
}

function photoCenterForPan(
  node: PhotoRenderNode,
  panX: number,
  panY: number,
  scale: number,
) {
  const ranges = photoPanRanges(node, scale);
  const centerU = positionFromNormalized(
    panX,
    ranges.minimumU,
    ranges.maximumU,
  );
  const centerV = positionFromNormalized(
    panY,
    ranges.minimumV,
    ranges.maximumV,
  );

  return {
    x:
      node.rotationCosine * centerU -
      node.rotationSine * centerV,
    y:
      node.rotationSine * centerU +
      node.rotationCosine * centerV,
  };
}

function normalizedPosition(
  position: number,
  minimum: number,
  maximum: number,
) {
  const span = maximum - minimum;
  if (span <= 0.0001) return 0;
  return clamp(((position - minimum) * 2) / span - 1, -1, 1);
}

function positionFromNormalized(
  normalized: number,
  minimum: number,
  maximum: number,
) {
  return (
    (minimum + maximum) / 2 +
    (clamp(normalized, -1, 1) * (maximum - minimum)) / 2
  );
}

function hexToNumber(value: string): number {
  return Number.parseInt(value.replace("#", ""), 16);
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}
