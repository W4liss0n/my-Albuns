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

const UM_TO_PX = 0.001;
const SHEET_GAP = 52;
const PRELOAD_MARGIN = 1;

interface AlbumCanvasProps {
  composition: CompositionPlan;
  selectedFrameId: string | null;
  focusedSheetId: string;
  viewport: ViewportState;
  onSelectFrame(frameId: string | null): void;
  onFocusSheet(sheetId: string): void;
  onViewportChange(viewport: ViewportState): void;
  onPanCommit(frameId: string, deltaX: number, deltaY: number): void;
  onZoomCommit(frameId: string, delta: number): void;
  onMaterializedChange(count: number): void;
}

interface DragGesture {
  frameId: string;
  startX: number;
  startY: number;
  frameWidth: number;
  frameHeight: number;
  photoLayer: Container;
  originalX: number;
  originalY: number;
}

interface ZoomGesture {
  frameId: string;
  delta: number;
  timer: number;
}

export function AlbumCanvas(props: AlbumCanvasProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const applicationRef = useRef<Application | null>(null);
  const dragRef = useRef<DragGesture | null>(null);
  const zoomGestureRef = useRef<ZoomGesture | null>(null);
  const propsRef = useRef(props);
  const [ready, setReady] = useState(false);
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
          "Canvas contínuo do Álbum. Use a roda para navegar e Ctrl mais roda para ampliar.",
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
      if (zoomGestureRef.current) {
        window.clearTimeout(zoomGestureRef.current.timer);
        zoomGestureRef.current = null;
      }
      destroyInitializedApp();
    };
  }, [props.composition.sheets.length]);

  useEffect(() => {
    const app = applicationRef.current;
    if (!app || !ready) return;

    const removed = app.stage.removeChildren();
    removed.forEach((child) => child.destroy({ children: true }));
    const world = new Container();
    app.stage.addChild(world);

    const sheetWidths = props.composition.sheets.map(
      (sheet) => sheet.widthUm * UM_TO_PX,
    );
    const sheetOffsets: number[] = [];
    let cursor = 0;
    for (const width of sheetWidths) {
      sheetOffsets.push(cursor);
      cursor += width + SHEET_GAP;
    }

    const viewportLeft = -props.viewport.offsetX / props.viewport.zoom;
    const viewportRight =
      viewportLeft + app.screen.width / props.viewport.zoom;
    const visibleIndexes = props.composition.sheets
      .map((sheet, index) => ({
        index,
        left: sheetOffsets[index],
        right: sheetOffsets[index] + sheet.widthUm * UM_TO_PX,
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
      Math.max(
        42,
        (app.screen.height -
          props.composition.sheets[0].heightUm *
            UM_TO_PX *
            props.viewport.zoom) /
          2,
      ),
    );
    world.scale.set(props.viewport.zoom);

    for (let index = firstVisible; index <= lastVisible; index += 1) {
      const sheet = props.composition.sheets[index];
      const sheetContainer = new Container();
      const width = sheet.widthUm * UM_TO_PX;
      const height = sheet.heightUm * UM_TO_PX;
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
      label.position.set(2, -24);
      sheetContainer.addChild(label);

      const centerLine = new Graphics()
        .moveTo(width / 2, 0)
        .lineTo(width / 2, height)
        .stroke({ color: 0x887b6c, width: 1, alpha: 0.32 });
      sheetContainer.addChild(centerLine);

      for (const frame of sheet.frames) {
        const frameContainer = new Container();
        const frameX = frame.clipRect.x * UM_TO_PX;
        const frameY = frame.clipRect.y * UM_TO_PX;
        const frameWidth = frame.clipRect.width * UM_TO_PX;
        const frameHeight = frame.clipRect.height * UM_TO_PX;
        frameContainer.position.set(frameX, frameY);
        frameContainer.eventMode = "static";
        frameContainer.hitArea = new Rectangle(
          0,
          0,
          frameWidth,
          frameHeight,
        );
        frameContainer.cursor = frame.photo ? "grab" : "pointer";

        const clip = new Graphics()
          .rect(0, 0, frameWidth, frameHeight)
          .fill(0xffffff);
        const photoLayer = new Container();
        if (frame.photo) {
          const drawWidth = frame.photo.drawRect.width * UM_TO_PX;
          const drawHeight = frame.photo.drawRect.height * UM_TO_PX;
          photoLayer.pivot.set(drawWidth / 2, drawHeight / 2);
          photoLayer.position.set(
            (frame.photo.drawRect.x - frame.clipRect.x) * UM_TO_PX +
              drawWidth / 2,
            (frame.photo.drawRect.y - frame.clipRect.y) * UM_TO_PX +
              drawHeight / 2,
          );
          photoLayer.rotation =
            (frame.photo.rotationDegrees * Math.PI) / 180;
          photoLayer.scale.x = frame.photo.mirrorX ? -1 : 1;
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
            .circle(drawWidth * 0.73, drawHeight * 0.28, drawHeight * 0.18)
            .fill({ color: 0xfff3d0, alpha: 0.32 });
          photoLayer.addChild(light);
          photoLayer.mask = clip;
          frameContainer.addChild(photoLayer, clip);
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
          if (!event.altKey || !frame.photo) return;
          event.stopPropagation();
          dragRef.current = {
            frameId: frame.frameId,
            startX: event.global.x,
            startY: event.global.y,
            frameWidth: frameWidth * props.viewport.zoom,
            frameHeight: frameHeight * props.viewport.zoom,
            photoLayer,
            originalX: photoLayer.x,
            originalY: photoLayer.y,
          };
          frameContainer.cursor = "grabbing";
        });
        frameContainer.on("wheel", (event: FederatedWheelEvent) => {
          if (!event.altKey || !frame.photo) return;
          event.preventDefault();
          const delta = event.deltaY < 0 ? 0.08 : -0.08;
          const current = zoomGestureRef.current;
          if (current) window.clearTimeout(current.timer);
          const accumulated =
            current?.frameId === frame.frameId ? current.delta + delta : delta;
          const timer = window.setTimeout(() => {
            const gesture = zoomGestureRef.current;
            if (gesture) {
              propsRef.current.onZoomCommit(
                gesture.frameId,
                gesture.delta,
              );
              zoomGestureRef.current = null;
            }
          }, 180);
          zoomGestureRef.current = {
            frameId: frame.frameId,
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
    app.stage.hitArea = new Rectangle(0, 0, app.screen.width, app.screen.height);
    app.stage.on("globalpointermove", (event: FederatedPointerEvent) => {
      const gesture = dragRef.current;
      if (!gesture) return;
      gesture.photoLayer.position.set(
        gesture.originalX +
          (event.global.x - gesture.startX) / props.viewport.zoom,
        gesture.originalY +
          (event.global.y - gesture.startY) / props.viewport.zoom,
      );
    });
    const finishDrag = (event: FederatedPointerEvent) => {
      const gesture = dragRef.current;
      if (!gesture) return;
      const deltaX =
        ((event.global.x - gesture.startX) / gesture.frameWidth) * 2;
      const deltaY =
        ((event.global.y - gesture.startY) / gesture.frameHeight) * 2;
      dragRef.current = null;
      propsRef.current.onPanCommit(gesture.frameId, deltaX, deltaY);
    };
    app.stage.on("pointerup", finishDrag);
    app.stage.on("pointerupoutside", finishDrag);

    const handleWheel = (event: WheelEvent) => {
      if (event.altKey) return;
      event.preventDefault();
      if (event.ctrlKey) {
        const direction = event.deltaY < 0 ? 0.08 : -0.08;
        propsRef.current.onViewportChange({
          ...propsRef.current.viewport,
          zoom: clamp(
            propsRef.current.viewport.zoom + direction,
            0.45,
            1.55,
          ),
        });
        return;
      }
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
    };
  }, [
    props.composition,
    props.focusedSheetId,
    props.onMaterializedChange,
    props.selectedFrameId,
    props.viewport,
    ready,
  ]);

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

function hexToNumber(value: string): number {
  return Number.parseInt(value.replace("#", ""), 16);
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}
