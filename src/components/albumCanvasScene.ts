import {
  Application,
  Container,
  FederatedPointerEvent,
  FederatedWheelEvent,
  Graphics,
  Rectangle,
  Sprite,
  Text,
  UPDATE_PRIORITY,
  type Texture,
} from "pixi.js";

import type {
  ComposedSheet,
  NormalizedPan,
} from "../domain/project";
import type {
  AlbumCanvasProps,
  CanvasMetrics,
  PhotoTransformDelta,
  PhotoTransformPreview,
} from "./albumCanvasContract";
import type {
  CanvasPerformanceTarget,
  CanvasPerformanceTargetState,
} from "./canvasPerformanceProbe";
import type {
  CanvasNavigationPerformanceTarget,
  CanvasNavigationRenderedFrame,
} from "./canvasNavigationPerformanceProbe";
import {
  CANVAS_VERTICAL_MARGIN_PX,
  continuousCanvasScale,
  type ContinuousCanvasLayout,
  MICROMETER_TO_CANVAS_PIXEL,
  SHEET_LABEL_HEIGHT_PX,
} from "./canvasGeometry";
import {
  createPhotoGeometry,
  type CanvasPhotoPlacement,
  type CanvasPoint,
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
import { ViewportTexturePool } from "./viewportTexturePool";

const PRELOAD_MARGIN = 1;
const VIEWPORT_PRELOAD_PX = 700;
const ZOOM_GESTURE_SETTLE_MS = 500;
const PAN_OUTSIDE_OPACITY = 0.24;

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
  pan: NormalizedPan;
  textureBacked: boolean;
}

interface SheetRenderNode {
  container: Container;
  signature: string;
  frameIds: string[];
  selectionOutlines: Map<string, Graphics>;
  focusOutline: Graphics;
}

interface PhotoPreviewLayerOptions {
  label: string;
  drawWidth: number;
  drawHeight: number;
  center: CanvasPoint;
  rotationDegrees: number;
  mirrorX: boolean;
  palette: readonly string[];
  previewTexture?: Texture;
}

interface DragGesture {
  generation: number;
  frameId: string;
  startX: number;
  startY: number;
  canvasScale: number;
  node: PhotoRenderNode;
  originalX: number;
  originalY: number;
  currentX: number;
  currentY: number;
  currentPan: NormalizedPan;
  currentZoom: number;
}

interface ZoomGestureRuntime {
  generation: number;
  gesture: PhotoZoomGesture;
  timer: number | null;
}

interface PendingNavigationProbe {
  generation: number;
  sheetId: string;
  resolve(value: CanvasNavigationRenderedFrame): void;
  reject(reason: unknown): void;
  signal?: AbortSignal;
  abortHandler?: () => void;
}

export class AlbumCanvasScene {
  private readonly world = new Container();
  private readonly sheetNodes = new Map<string, SheetRenderNode>();
  private readonly photoNodes = new Map<string, PhotoRenderNode>();
  private input: AlbumCanvasProps | null = null;
  private projectId: string | null = null;
  private projectGeneration = 0;
  private canvasScale = 1;
  private drag: DragGesture | null = null;
  private zoomGesture: ZoomGestureRuntime | null = null;
  private readonly pendingCommitFrames = new Set<string>();
  private externalPreviewFrameId: string | null = null;
  private lastCanvasMetrics: CanvasMetrics | null = null;
  private pendingNavigationProbe: PendingNavigationProbe | null = null;
  private readonly previewTextures: ViewportTexturePool;

  constructor(
    private readonly app: Application,
    onPreviewTextureError: () => void = () => undefined,
    private readonly onPreviewTextureChange: () => void = () => undefined,
  ) {
    this.previewTextures = new ViewportTexturePool(
      this.refreshAfterPreviewTextureChange,
      onPreviewTextureError,
    );
    this.world.label = "album-world";
    this.app.stage.addChild(this.world);
    this.app.stage.eventMode = "static";
    this.app.stage.on(
      "globalpointermove",
      this.handleGlobalPointerMove,
    );
    this.app.stage.on("pointerup", this.finishDrag);
    this.app.stage.on("pointerupoutside", this.finishDrag);
    this.app.stage.on("pointercancel", this.cancelDrag);
    this.app.canvas.addEventListener("wheel", this.handleCanvasWheel, {
      passive: false,
    });
  }

  update(input: AlbumCanvasProps, hostHeight: number) {
    if (this.projectId !== input.projectId) {
      this.resetProjectScene();
      this.projectId = input.projectId;
      this.projectGeneration += 1;
    }
    this.input = input;
    const sheets = input.composition.sheets;
    const firstSheet = sheets[0];
    if (!firstSheet) {
      this.clearMaterializedSheets();
      this.previewTextures.sync([]);
      return;
    }
    const layout = input.continuousCanvasLayout;

    const sheetHeight =
      firstSheet.heightUm * MICROMETER_TO_CANVAS_PIXEL;
    const scale = continuousCanvasScale(
      hostHeight || this.app.screen.height,
      sheetHeight,
    );
    this.canvasScale = scale;
    const boundedOffsetX = layout.clampOffset(
      input.viewport.offsetX,
      scale,
      this.app.screen.width,
    );
    if (Math.abs(boundedOffsetX - input.viewport.offsetX) > 0.0001) {
      input.onViewportChange({
        ...input.viewport,
        offsetX: boundedOffsetX,
      });
    }

    this.synchronizeCenteredSheet(layout, boundedOffsetX, scale);
    this.reportCanvasMetrics(scale);
    this.world.position.set(
      boundedOffsetX,
      CANVAS_VERTICAL_MARGIN_PX +
        SHEET_LABEL_HEIGHT_PX * scale,
    );
    this.world.scale.set(scale);
    this.app.stage.hitArea = new Rectangle(
      0,
      0,
      this.app.screen.width,
      this.app.screen.height,
    );

    this.reconcileMaterializedSheets(layout, boundedOffsetX, scale);
    this.settleNavigationProbe(layout, boundedOffsetX, scale);
    this.updateDecorations();
    this.applyExternalPhotoZoomPreview();
  }

  resize(hostHeight: number) {
    this.app.resize();
    if (this.input) this.update(this.input, hostHeight);
  }

  destroy() {
    this.rejectNavigationProbe(
      new DOMException(
        "O Canvas foi encerrado durante o probe.",
        "AbortError",
      ),
    );
    this.resetTransientInteractions();
    this.app.canvas.removeEventListener(
      "wheel",
      this.handleCanvasWheel,
    );
    this.app.stage.removeAllListeners();
    this.clearMaterializedSheets();
    this.previewTextures.destroy();
  }

  performanceTarget(): CanvasPerformanceTargetState {
    const input = this.input;
    if (!input || !this.previewTextures.isSettled()) {
      return { status: "pending" };
    }
    let node: PhotoRenderNode | undefined;
    for (const sheet of input.composition.sheets) {
      for (const frame of sheet.frames) {
        const candidate = this.photoNodes.get(frame.frameId);
        if (candidate?.textureBacked) {
          node = candidate;
          break;
        }
      }
      if (node) break;
    }
    if (!node) {
      return {
        status: "failed",
        reason: "texture_unavailable",
      };
    }
    const generation = this.projectGeneration;
    const assertCurrent = () => {
      if (
        generation !== this.projectGeneration ||
        this.photoNodes.get(node.frameId) !== node
      ) {
        throw new DOMException(
          "O alvo do probe do Canvas deixou de existir.",
          "AbortError",
        );
      }
    };
    const preview = (
      pan: NormalizedPan,
      zoom: number,
      placement: CanvasPhotoPlacement,
    ) => {
      assertCurrent();
      applyPhotoPlacementPreview(node, zoom, placement);
      input.onTransformPreview(
        createTransformPreview(node.frameId, pan, zoom),
      );
    };

    const target: CanvasPerformanceTarget = {
      frameId: node.frameId,
      textureBacked: true,
      previewPan: (amount) => {
        assertCurrent();
        setPhotoPanAids(node, true);
        const amplitude =
          Math.min(
            node.geometry.current.size.width,
            node.geometry.current.size.height,
          ) * 0.18;
        const constrained = node.geometry.constrain(
          {
            x: node.geometry.current.center.x + amplitude * amount,
            y:
              node.geometry.current.center.y +
              amplitude * amount * 0.45,
          },
          node.baseZoom,
        );
        preview(
          constrained.pan,
          constrained.zoom,
          constrained.placement,
        );
      },
      previewZoom: (amount) => {
        assertCurrent();
        setPhotoPanAids(node, false);
        const zoomSpan = Math.min(
          0.6,
          node.geometry.zoomRange.maximum - node.baseZoom,
        );
        const zoomed = node.geometry.zoom(
          node.baseZoom + zoomSpan * amount,
        );
        preview(node.pan, zoomed.zoom, zoomed.placement);
      },
      nextRenderedFrame: () => {
        assertCurrent();
        return new Promise<number>((resolve) => {
          this.app.ticker.addOnce(
            () => resolve(performance.now()),
            undefined,
            UPDATE_PRIORITY.UTILITY,
          );
        });
      },
      reset: () => {
        if (
          generation !== this.projectGeneration ||
          this.photoNodes.get(node.frameId) !== node
        ) {
          return;
        }
        setPhotoPanAids(node, false);
        resetPhotoPreview(node);
        input.onTransformPreview(null);
      },
    };
    return { status: "ready", target };
  }

  navigationPerformanceTarget(
    navigateToSheet: (sheetId: string) => void,
  ): CanvasNavigationPerformanceTarget | null {
    const input = this.input;
    if (!input) return null;
    const generation = this.projectGeneration;
    const sheetIds = input.composition.sheets.map((sheet) => sheet.sheetId);

    return {
      sheetIds,
      navigateToSheet: (sheetId, signal) => {
        if (
          generation !== this.projectGeneration ||
          !sheetIds.includes(sheetId)
        ) {
          return Promise.reject(
            new DOMException(
              "O alvo do probe de navegação deixou de existir.",
              "AbortError",
            ),
          );
        }
        this.rejectNavigationProbe(
          new DOMException(
            "O probe iniciou outra navegação antes de concluir a anterior.",
            "AbortError",
          ),
        );

        return new Promise<CanvasNavigationRenderedFrame>(
          (resolve, reject) => {
            const pending: PendingNavigationProbe = {
              generation,
              sheetId,
              resolve,
              reject,
              signal,
            };
            if (signal) {
              pending.abortHandler = () => {
                if (this.pendingNavigationProbe === pending) {
                  this.rejectNavigationProbe(
                    new DOMException(
                      "O probe do Canvas foi cancelado.",
                      "AbortError",
                    ),
                  );
                }
              };
              signal.addEventListener("abort", pending.abortHandler, {
                once: true,
              });
            }
            this.pendingNavigationProbe = pending;
            try {
              navigateToSheet(sheetId);
            } catch (error: unknown) {
              this.rejectNavigationProbe(error);
            }
          },
        );
      },
    };
  }

  private resetProjectScene() {
    this.rejectNavigationProbe(
      new DOMException(
        "O Projeto mudou durante o probe de navegação.",
        "AbortError",
      ),
    );
    this.resetTransientInteractions();
    this.clearMaterializedSheets();
    this.previewTextures.sync([]);
    this.lastCanvasMetrics = null;
  }

  private resetTransientInteractions() {
    if (this.zoomGesture?.timer != null) {
      window.clearTimeout(this.zoomGesture.timer);
    }
    if (this.drag) {
      setPhotoPanAids(this.drag.node, false);
      resetPhotoPreview(this.drag.node);
    }
    if (this.zoomGesture) {
      const zoomNode = this.photoNodes.get(
        this.zoomGesture.gesture.frameId,
      );
      if (zoomNode && zoomNode !== this.drag?.node) {
        resetPhotoPreview(zoomNode);
      }
    }
    if (this.externalPreviewFrameId) {
      const externalNode = this.photoNodes.get(
        this.externalPreviewFrameId,
      );
      if (externalNode) resetPhotoPreview(externalNode);
    }
    this.zoomGesture = null;
    this.drag = null;
    this.externalPreviewFrameId = null;
    this.pendingCommitFrames.clear();
  }

  private synchronizeCenteredSheet(
    layout: ContinuousCanvasLayout,
    offsetX: number,
    scale: number,
  ) {
    if (!this.input) return;
    const centeredSheetId = layout.centeredSheetId(
      offsetX,
      scale,
      this.app.screen.width,
    );
    if (
      centeredSheetId &&
      centeredSheetId !== this.input.centeredSheetId
    ) {
      this.input.onCenteredSheetChange(centeredSheetId);
    }
  }

  private reportCanvasMetrics(scale: number) {
    if (!this.input) return;
    const metrics = {
      width: this.app.screen.width,
      scale,
    };
    const previous = this.lastCanvasMetrics;
    if (
      previous === null ||
      Math.abs(previous.width - metrics.width) > 0.0001 ||
      Math.abs(previous.scale - metrics.scale) > 0.0001
    ) {
      this.lastCanvasMetrics = metrics;
      this.input.onCanvasMetricsChange?.(metrics);
    }
  }

  private reconcileMaterializedSheets(
    layout: ContinuousCanvasLayout,
    boundedOffsetX: number,
    scale: number,
  ) {
    if (!this.input) return;
    const sheets = this.input.composition.sheets;
    const viewportLeft = -boundedOffsetX / scale;
    const viewportRight =
      viewportLeft + this.app.screen.width / scale;
    const visibleIndexes = layout.entries
      .filter(
        ({ left, right }) =>
          right >= viewportLeft - VIEWPORT_PRELOAD_PX &&
          left <= viewportRight + VIEWPORT_PRELOAD_PX,
      )
      .map(({ index }) => index);
    const firstVisible = Math.max(
      0,
      (visibleIndexes[0] ?? 0) - PRELOAD_MARGIN,
    );
    const lastVisible = Math.min(
      sheets.length - 1,
      (visibleIndexes[visibleIndexes.length - 1] ?? 0) +
        PRELOAD_MARGIN,
    );
    const desiredSheets = sheets.slice(firstVisible, lastVisible + 1);
    const desiredIds = new Set(desiredSheets.map((sheet) => sheet.sheetId));
    const desiredPreviewUrls = new Set<string>();
    const signatures = new Map<string, string>();

    for (const sheet of desiredSheets) {
      const previewStates = sheet.frames.map((frame) => {
        if (!frame.photo) return null;
        const url =
          this.input?.mediaPreviewUrls?.[frame.photo.mediaId] ?? null;
        if (url) desiredPreviewUrls.add(url);
        return url
          ? [url, this.previewTextures.get(url) !== undefined]
          : null;
      });
      signatures.set(
        sheet.sheetId,
        JSON.stringify([sheet, previewStates]),
      );
    }

    for (const [sheetId, node] of this.sheetNodes) {
      if (
        !desiredIds.has(sheetId) ||
        node.signature !== signatures.get(sheetId)
      ) {
        this.removeSheetNode(sheetId, node);
      }
    }
    this.previewTextures.sync(desiredPreviewUrls);

    for (let index = firstVisible; index <= lastVisible; index += 1) {
      const sheet = sheets[index];
      const signature = signatures.get(sheet.sheetId) ?? "";
      let node = this.sheetNodes.get(sheet.sheetId);
      if (!node) {
        node = this.createSheetNode(sheet, signature);
        this.sheetNodes.set(sheet.sheetId, node);
        this.world.addChild(node.container);
      }
      node.container.position.set(layout.entries[index].left, 0);
    }
  }

  private settleNavigationProbe(
    layout: ContinuousCanvasLayout,
    boundedOffsetX: number,
    scale: number,
  ) {
    const pending = this.pendingNavigationProbe;
    const input = this.input;
    if (!pending || !input) return;
    if (
      pending.generation !== this.projectGeneration ||
      pending.signal?.aborted
    ) {
      this.rejectNavigationProbe(
        new DOMException(
          "O probe do Canvas foi cancelado.",
          "AbortError",
        ),
      );
      return;
    }
    if (
      layout.centeredSheetId(
        boundedOffsetX,
        scale,
        this.app.screen.width,
      ) !== pending.sheetId
    ) {
      return;
    }
    const sheet = input.composition.sheets.find(
      ({ sheetId }) => sheetId === pending.sheetId,
    );
    const node = this.sheetNodes.get(pending.sheetId);
    if (!sheet || !node || !this.previewTextures.isSettled()) return;

    const texturedFrameIds = sheet.frames
      .filter(
        (frame) =>
          frame.photo &&
          input.mediaPreviewUrls?.[frame.photo.mediaId] !== undefined,
      )
      .map((frame) => frame.frameId);
    if (
      texturedFrameIds.length === 0 ||
      texturedFrameIds.some(
        (frameId) => !this.photoNodes.get(frameId)?.textureBacked,
      )
    ) {
      this.rejectNavigationProbe(
        new Error(
          "A Lâmina de destino não possui uma textura real do Cache.",
        ),
      );
      return;
    }

    const renderedFrame = {
      residentSheetCount: this.sheetNodes.size,
      residentTextureCount: this.previewTextures.loadedCount(),
    };
    this.pendingNavigationProbe = null;
    if (pending.signal && pending.abortHandler) {
      pending.signal.removeEventListener("abort", pending.abortHandler);
    }
    this.app.ticker.addOnce(
      () =>
        pending.resolve({
          ...renderedFrame,
          renderedAt: performance.now(),
        }),
      undefined,
      UPDATE_PRIORITY.UTILITY,
    );
  }

  private rejectNavigationProbe(reason: unknown) {
    const pending = this.pendingNavigationProbe;
    if (!pending) return;
    this.pendingNavigationProbe = null;
    if (pending.signal && pending.abortHandler) {
      pending.signal.removeEventListener("abort", pending.abortHandler);
    }
    pending.reject(reason);
  }

  private clearMaterializedSheets() {
    for (const [sheetId, node] of this.sheetNodes) {
      this.removeSheetNode(sheetId, node);
    }
  }

  private removeSheetNode(sheetId: string, node: SheetRenderNode) {
    this.world.removeChild(node.container);
    node.container.destroy({ children: true });
    this.sheetNodes.delete(sheetId);
    for (const frameId of node.frameIds) {
      this.photoNodes.delete(frameId);
    }
  }

  private createSheetNode(
    sheet: ComposedSheet,
    signature: string,
  ): SheetRenderNode {
    const sheetContainer = new Container();
    const width = sheet.widthUm * MICROMETER_TO_CANVAS_PIXEL;
    const height = sheet.heightUm * MICROMETER_TO_CANVAS_PIXEL;
    sheetContainer.eventMode = "static";
    sheetContainer.hitArea = new Rectangle(0, 0, width, height);
    sheetContainer.cursor = "default";
    sheetContainer.on(
      "pointertap",
      (event: FederatedPointerEvent) => {
        if (event.target === sheetContainer && this.input) {
          this.input.onSelectFrame(null);
          this.input.onFocusSheet(sheet.sheetId);
        }
      },
    );

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

    const selectionOutlines = new Map<string, Graphics>();
    const frameIds: string[] = [];
    for (const frame of sheet.frames) {
      frameIds.push(frame.frameId);
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
        const previewOptions = {
          drawWidth: geometry.current.size.width,
          drawHeight: geometry.current.size.height,
          center: geometry.current.center,
          rotationDegrees: frame.photo.rotationDegrees,
          mirrorX: frame.photo.mirrorX,
          palette: frame.photo.palette,
          previewTexture: this.previewTextureFor(
            frame.photo.mediaId,
          ),
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
          textureBacked: previewOptions.previewTexture !== undefined,
        };
        this.photoNodes.set(frame.frameId, photoNode);
      } else {
        frameContainer.addChild(
          createPlaceholder(frameWidth, frameHeight),
          createPlaceholderCross(frameWidth, frameHeight),
        );
      }

      const outline = new Graphics()
        .rect(0, 0, frameWidth, frameHeight)
        .stroke({
          color: hexToNumber(SHEET_VISUAL_STYLE.frame.outline),
          width: SHEET_VISUAL_STYLE.frame.outlineWidthPx,
          alpha: SHEET_VISUAL_STYLE.frame.outlineOpacity,
        });
      const selectionOutline = new Graphics()
        .rect(0, 0, frameWidth, frameHeight)
        .stroke({ color: 0xb8874f, width: 3, alpha: 1 });
      selectionOutline.label = `frame-selection-${frame.frameId}`;
      selectionOutline.eventMode = "none";
      selectionOutline.visible = false;
      selectionOutlines.set(frame.frameId, selectionOutline);
      frameContainer.addChild(outline, selectionOutline);

      frameContainer.on(
        "pointertap",
        (event: FederatedPointerEvent) => {
          event.stopPropagation();
          if (!event.altKey && this.input) {
            this.input.onSelectFrame(frame.frameId);
            this.input.onFocusSheet(sheet.sheetId);
          }
        },
      );
      frameContainer.on(
        "pointerdown",
        (event: FederatedPointerEvent) => {
          if (!event.altKey || !photoNode) return;
          event.stopPropagation();
          this.startPanGesture(frameContainer, photoNode, event);
        },
      );
      frameContainer.on(
        "wheel",
        (event: FederatedWheelEvent) => {
          if (!event.altKey || !photoNode) return;
          this.handlePhotoWheel(photoNode, event);
        },
      );
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

    const focusOutline = new Graphics()
      .roundRect(-5, -5, width + 10, height + 10, 7)
      .stroke({ color: 0xc99a5d, width: 2, alpha: 0.9 });
    focusOutline.label = `sheet-focus-${sheet.sheetId}`;
    focusOutline.eventMode = "none";
    focusOutline.visible = false;
    sheetContainer.addChild(focusOutline);

    return {
      container: sheetContainer,
      signature,
      frameIds,
      selectionOutlines,
      focusOutline,
    };
  }

  private previewTextureFor(mediaId: string) {
    const url = this.input?.mediaPreviewUrls?.[mediaId];
    return url ? this.previewTextures.get(url) : undefined;
  }

  private readonly refreshAfterPreviewTextureChange = () => {
    if (this.input) {
      this.update(this.input, this.app.screen.height);
    }
    this.onPreviewTextureChange();
  };

  private updateDecorations() {
    if (!this.input) return;
    for (const [sheetId, node] of this.sheetNodes) {
      node.focusOutline.visible =
        sheetId === this.input.focusedSheetId;
      for (const [frameId, outline] of node.selectionOutlines) {
        outline.visible = frameId === this.input.selectedFrameId;
      }
    }
  }

  private startPanGesture(
    frameContainer: Container,
    photoNode: PhotoRenderNode,
    event: FederatedPointerEvent,
  ) {
    if (this.pendingCommitFrames.has(photoNode.frameId)) return;
    const activeZoom = this.zoomGesture;
    const continuesZoom =
      activeZoom?.gesture.frameId === photoNode.frameId;
    if (continuesZoom && activeZoom.timer !== null) {
      window.clearTimeout(activeZoom.timer);
      activeZoom.timer = null;
    }
    this.drag = {
      generation: this.projectGeneration,
      frameId: photoNode.frameId,
      startX: event.global.x,
      startY: event.global.y,
      canvasScale: this.canvasScale,
      node: photoNode,
      originalX: photoNode.layer.x,
      originalY: photoNode.layer.y,
      currentX: photoNode.layer.x,
      currentY: photoNode.layer.y,
      currentPan: photoNode.pan,
      currentZoom: continuesZoom
        ? activeZoom.gesture.baseZoom + activeZoom.gesture.delta
        : photoNode.baseZoom,
    };
    setPhotoPanAids(photoNode, true);
    frameContainer.cursor = "grabbing";
  }

  private handlePhotoWheel(
    photoNode: PhotoRenderNode,
    event: FederatedWheelEvent,
  ) {
    if (!this.input) return;
    event.preventDefault();
    if (this.pendingCommitFrames.has(photoNode.frameId)) return;
    const current = this.zoomGesture;
    if (current?.timer != null) {
      window.clearTimeout(current.timer);
    }
    const transition = advancePhotoZoomGesture(
      current?.gesture ?? null,
      {
        frameId: photoNode.frameId,
        baseZoom: photoNode.baseZoom,
        zoomRange: photoNode.geometry.zoomRange,
        wheelDeltaY: event.deltaY,
      },
    );
    if (transition.interruptedCommit) {
      const interruptedNode = this.photoNodes.get(
        transition.interruptedCommit.frameId,
      );
      if (interruptedNode) {
        this.commitPhotoTransform(
          {
            frameId: transition.interruptedCommit.frameId,
            deltaPanX: 0,
            deltaPanY: 0,
            deltaZoom: transition.interruptedCommit.delta,
          },
          interruptedNode,
          current?.generation ?? this.projectGeneration,
        );
      }
    }

    const activeDrag = this.drag;
    if (activeDrag?.frameId === photoNode.frameId) {
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
      this.input.onTransformPreview(
        createTransformPreview(
          activeDrag.frameId,
          activeDrag.currentPan,
          activeDrag.currentZoom,
        ),
      );
      this.zoomGesture = {
        generation: this.projectGeneration,
        gesture: transition.gesture,
        timer: null,
      };
      return;
    }

    applyPhotoZoomPreview(photoNode, transition.previewZoom);
    this.input.onTransformPreview(
      createTransformPreview(
        photoNode.frameId,
        photoNode.pan,
        transition.previewZoom,
      ),
    );
    const runtime: ZoomGestureRuntime = {
      generation: this.projectGeneration,
      gesture: transition.gesture,
      timer: null,
    };
    runtime.timer = window.setTimeout(() => {
      if (
        this.zoomGesture !== runtime ||
        runtime.generation !== this.projectGeneration
      ) {
        return;
      }
      this.zoomGesture = null;
      const commit = finishPhotoZoomGesture(runtime.gesture);
      const commitNode = commit
        ? this.photoNodes.get(commit.frameId)
        : null;
      if (commit && commitNode) {
        this.commitPhotoTransform(
          {
            frameId: commit.frameId,
            deltaPanX: 0,
            deltaPanY: 0,
            deltaZoom: commit.delta,
          },
          commitNode,
          runtime.generation,
        );
      } else {
        this.input?.onTransformPreview(null);
      }
    }, ZOOM_GESTURE_SETTLE_MS);
    this.zoomGesture = runtime;
  }

  private readonly handleGlobalPointerMove = (
    event: FederatedPointerEvent,
  ) => {
    if (
      !this.drag ||
      !this.input ||
      this.drag.generation !== this.projectGeneration
    ) {
      return;
    }
    updatePanPreview(this.drag, event.global.x, event.global.y);
    this.input.onTransformPreview(
      createTransformPreview(
        this.drag.frameId,
        this.drag.currentPan,
        this.drag.currentZoom,
      ),
    );
  };

  private readonly finishDrag = (event: FederatedPointerEvent) => {
    const gesture = this.drag;
    if (
      !gesture ||
      !this.input ||
      gesture.generation !== this.projectGeneration
    ) {
      return;
    }
    updatePanPreview(gesture, event.global.x, event.global.y);
    this.drag = null;
    setPhotoPanAids(gesture.node, false);

    const deltaPanX = gesture.currentPan.x - gesture.node.pan.x;
    const deltaPanY = gesture.currentPan.y - gesture.node.pan.y;
    const deltaZoom = gesture.currentZoom - gesture.node.baseZoom;
    const combinedZoom = this.zoomGesture;
    const ownsCombinedZoom =
      combinedZoom?.gesture.frameId === gesture.frameId;
    const changedPan =
      Math.abs(deltaPanX) > 0.0001 ||
      Math.abs(deltaPanY) > 0.0001;
    const changedZoom = Math.abs(deltaZoom) > 0.0001;

    if (ownsCombinedZoom) {
      if (combinedZoom.timer !== null) {
        window.clearTimeout(combinedZoom.timer);
      }
      this.zoomGesture = null;
    }

    if ((ownsCombinedZoom && changedZoom) || changedPan) {
      this.input.onTransformPreview(
        createTransformPreview(
          gesture.frameId,
          gesture.currentPan,
          gesture.currentZoom,
        ),
      );
      this.commitPhotoTransform(
        {
          frameId: gesture.frameId,
          deltaPanX,
          deltaPanY,
          deltaZoom: ownsCombinedZoom ? deltaZoom : 0,
        },
        gesture.node,
        gesture.generation,
      );
    } else {
      this.input.onTransformPreview(null);
    }
  };

  private commitPhotoTransform(
    delta: PhotoTransformDelta,
    node: PhotoRenderNode,
    generation: number,
  ) {
    const input = this.input;
    if (
      !input ||
      generation !== this.projectGeneration ||
      this.pendingCommitFrames.has(delta.frameId)
    ) {
      return;
    }

    this.pendingCommitFrames.add(delta.frameId);
    let result: Promise<boolean>;
    try {
      result = input.onTransformCommit(delta);
    } catch {
      this.settlePhotoTransform(
        delta.frameId,
        node,
        generation,
        false,
      );
      return;
    }
    void result.then(
      (accepted) =>
        this.settlePhotoTransform(
          delta.frameId,
          node,
          generation,
          accepted,
        ),
      () =>
        this.settlePhotoTransform(
          delta.frameId,
          node,
          generation,
          false,
        ),
    );
  }

  private settlePhotoTransform(
    frameId: string,
    node: PhotoRenderNode,
    generation: number,
    accepted: boolean,
  ) {
    if (generation !== this.projectGeneration) return;
    this.pendingCommitFrames.delete(frameId);
    if (!accepted && this.photoNodes.get(frameId) === node) {
      resetPhotoPreview(node);
      this.input?.onTransformPreview(null);
    }
  }

  private readonly cancelDrag = () => {
    const gesture = this.drag;
    if (!gesture) return;
    this.drag = null;
    setPhotoPanAids(gesture.node, false);
    resetPhotoPreview(gesture.node);
    this.input?.onTransformPreview(null);

    const combinedZoom = this.zoomGesture;
    if (combinedZoom?.gesture.frameId === gesture.frameId) {
      if (combinedZoom.timer !== null) {
        window.clearTimeout(combinedZoom.timer);
      }
      this.zoomGesture = null;
    }
  };

  private readonly handleCanvasWheel = (event: WheelEvent) => {
    if (!this.input || event.altKey) return;
    event.preventDefault();
    if (event.ctrlKey) return;
    const layout = this.input.continuousCanvasLayout;
    const nextOffset = layout.clampOffset(
      this.input.viewport.offsetX -
        (event.deltaX || event.deltaY) * 0.9,
      this.canvasScale,
      this.app.screen.width,
    );
    this.input.onViewportChange({
      ...this.input.viewport,
      offsetX: nextOffset,
    });
    this.synchronizeCenteredSheet(
      layout,
      nextOffset,
      this.canvasScale,
    );
  };

  private applyExternalPhotoZoomPreview() {
    if (!this.input) return;
    const previousFrameId = this.externalPreviewFrameId;
    const nextPreview = this.input.photoZoomPreview;
    if (previousFrameId && previousFrameId !== nextPreview?.frameId) {
      const previousNode = this.photoNodes.get(previousFrameId);
      if (previousNode) resetPhotoPreview(previousNode);
    }

    if (nextPreview) {
      const nextNode = this.photoNodes.get(nextPreview.frameId);
      if (nextNode) applyPhotoZoomPreview(nextNode, nextPreview.value);
      this.externalPreviewFrameId = nextPreview.frameId;
    } else {
      if (previousFrameId) {
        const previousNode = this.photoNodes.get(previousFrameId);
        if (previousNode) resetPhotoPreview(previousNode);
      }
      this.externalPreviewFrameId = null;
    }
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
  previewTexture,
}: PhotoPreviewLayerOptions) {
  const photoLayer = new Container();
  photoLayer.label = label;
  photoLayer.pivot.set(drawWidth / 2, drawHeight / 2);
  photoLayer.position.set(center.x, center.y);
  photoLayer.rotation = (rotationDegrees * Math.PI) / 180;
  photoLayer.scale.set(mirrorX ? -1 : 1, 1);

  if (previewTexture) {
    const sprite = new Sprite({
      texture: previewTexture,
      width: drawWidth,
      height: drawHeight,
    });
    photoLayer.addChild(sprite);
    return photoLayer;
  }

  const photoStyle = SHEET_VISUAL_STYLE.photo;
  for (let stripe = 0; stripe < photoStyle.stripeCount; stripe += 1) {
    const paletteIndex = photoPaletteIndexForStripe(stripe);
    photoLayer.addChild(
      new Graphics()
        .rect(
          (drawWidth / photoStyle.stripeCount) * stripe,
          0,
          drawWidth / photoStyle.stripeCount +
            photoStyle.stripeOverlapPx,
          drawHeight,
        )
        .fill({
          color: hexToNumber(palette[paletteIndex]),
        }),
    );
  }
  photoLayer.addChild(
    new Graphics()
      .circle(
        drawWidth * photoStyle.lightCenterXRatio,
        drawHeight * photoStyle.lightCenterYRatio,
        drawHeight * photoStyle.lightRadiusToHeightRatio,
      )
      .fill({
        color: hexToNumber(photoStyle.lightColor),
        alpha: photoStyle.lightOpacity,
      }),
  );
  return photoLayer;
}

function createPlaceholder(frameWidth: number, frameHeight: number) {
  return new Graphics()
    .rect(0, 0, frameWidth, frameHeight)
    .fill({
      color: hexToNumber(SHEET_VISUAL_STYLE.placeholder.fill),
    })
    .stroke({
      color: hexToNumber(SHEET_VISUAL_STYLE.placeholder.outline),
      width: SHEET_VISUAL_STYLE.placeholder.outlineWidthPx,
      alpha: SHEET_VISUAL_STYLE.placeholder.outlineOpacity,
    });
}

function createPlaceholderCross(
  frameWidth: number,
  frameHeight: number,
) {
  const style = SHEET_VISUAL_STYLE.placeholder;
  return new Graphics()
    .moveTo(
      frameWidth / 2 - style.crossHalfLengthPx,
      frameHeight / 2,
    )
    .lineTo(
      frameWidth / 2 + style.crossHalfLengthPx,
      frameHeight / 2,
    )
    .moveTo(
      frameWidth / 2,
      frameHeight / 2 - style.crossHalfLengthPx,
    )
    .lineTo(
      frameWidth / 2,
      frameHeight / 2 + style.crossHalfLengthPx,
    )
    .stroke({
      color: hexToNumber(style.crossColor),
      width: style.crossWidthPx,
      alpha: style.crossOpacity,
    });
}

function createTransformPreview(
  frameId: string,
  pan: NormalizedPan,
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
  placement: CanvasPhotoPlacement,
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
