import {
  Application,
  Container,
  Rectangle,
  type Ticker,
} from "pixi.js";

import type { ComposedSheet } from "../domain/project";
import type {
  AlbumCanvasProps,
  CanvasMetrics,
  SheetBarMetadata,
} from "./albumCanvasContract";
import {
  CANVAS_VERTICAL_MARGIN_PX,
  continuousCanvasScale,
  createContinuousCanvasLayout,
  type ContinuousCanvasLayout,
  MICROMETER_TO_CANVAS_PIXEL,
} from "./canvasGeometry";
import {
  applyPlaceholderLabelScale,
  createSheetRenderNode,
  destroySheetRenderNode,
  type PhotoRenderNode,
  type SheetRenderNode,
} from "./albumCanvasRenderNodes";
import { applyFrameSelectionScale, createFrameSelectionRenderNode } from "./frameSelectionRenderNode";
import {
  albumCanvasModePolicy,
  sheetsForCanvasMode,
} from "./albumCanvasMode";
import { applySheetBarScale, setSheetBarOverlayHovered, setSheetBarSwapFocused } from "./sheetBarRenderNode";
import { PhotoInteractionSession } from "./photoInteractionSession";
import { FrameInteractionSession } from "./frameInteractionSession";
import { FrameContentDragSession } from "./frameContentDragSession";
import { FrameContentDragVisual } from "./frameContentDragVisual";
import { ViewportTexturePool } from "./viewportTexturePool";

const PRELOAD_MARGIN = 1;
const VIEWPORT_PRELOAD_PX = 700;
const SHEET_REORDER_TRANSITION_MS = 140;

interface SheetPositionAnimation {
  readonly fromX: number;
  readonly toX: number;
  elapsedMs: number;
}

interface BarSheetReorderPreview {
  readonly draggedSheetId: string;
  readonly sheets: readonly ComposedSheet[];
}

export class AlbumCanvasScene {
  private readonly world = new Container();
  private readonly sheetNodes = new Map<string, SheetRenderNode>();
  private hoveredBar: { sheetId: string; swapHovered: boolean } | null = null;
  private focusedBarSheetId: string | null = null;
  private readonly photoNodes = new Map<string, PhotoRenderNode>();
  private input: AlbumCanvasProps | null = null;
  private projectId: string | null = null;
  private modeSignature: string | null = null;
  private projectGeneration = 0;
  private canvasScale = 1;
  private lastCanvasMetrics: CanvasMetrics | null = null;
  private lastMediaDemandSignature: string | null = null;
  private pendingViewportOffsetX: number | null = null;
  private sheetReorderPreviewActive = false;
  private sheetReorderPlaceholderSheetId: string | null = null;
  private sheetPositionTickerAttached = false;
  private readonly sheetPositionAnimations = new Map<
    string,
    SheetPositionAnimation
  >();
  private readonly previewTextures: ViewportTexturePool;
  private readonly photoInteractions: PhotoInteractionSession;
  private readonly frameInteractions: FrameInteractionSession;
  private readonly frameContentDrag: FrameContentDragSession;
  private readonly frameContentDragVisual: FrameContentDragVisual;

  constructor(
    private readonly app: Application,
    onPreviewTextureError: () => void = () => undefined,
    private readonly onPreviewTextureChange: () => void = () => undefined,
    onPreviewTextureLoad: (url: string) => void = () => undefined,
  ) {
    this.previewTextures = new ViewportTexturePool(
      this.refreshAfterPreviewTextureChange,
      onPreviewTextureError,
      onPreviewTextureLoad,
    );
    this.photoInteractions = new PhotoInteractionSession(
      this.photoNodes,
      () => ({
        input: this.input,
        projectGeneration: this.projectGeneration,
        canvasScale: this.canvasScale,
      }),
    );
    this.frameInteractions = new FrameInteractionSession(
      app.canvas,
      () => ({ input: this.input, canvasScale: this.canvasScale, screen: app.screen }),
      () => { if (this.input) this.update(this.input, app.screen.height); },
    );
    this.frameContentDrag = new FrameContentDragSession(
      app.canvas, () => this.input,
      (x, y) => this.resolvePhotoDropPoint(x, y),
      (delta) => this.scrollContinuousCanvas(delta),
      () => { if (this.input) this.updateDecorations(this.input.composition.sheets); },
    );
    this.world.label = "album-world";
    this.app.stage.addChild(this.world);
    this.frameContentDragVisual = new FrameContentDragVisual(app, this.photoNodes);
    this.app.stage.eventMode = "static";
    this.app.stage.hitArea = this.app.screen;
    this.app.stage.on("rightclick", (event) => {
      if (event.target !== this.app.stage || this.input?.mode.kind !== "sheet-editing") return;
      this.openEmptyCanvasContextMenu(this.input.mode.sheetId, { x: event.clientX, y: event.clientY });
    });
    this.app.stage.on(
      "globalpointermove",
      this.photoInteractions.handlePointerMove,
    );
    this.app.stage.on("pointerup", this.photoInteractions.finishPan);
    this.app.stage.on("pointerupoutside", this.photoInteractions.finishPan);
    this.app.stage.on("pointercancel", this.photoInteractions.cancelPan);
  }

  handleSheetBarHover(sheetId: string, hovered: boolean, swapHovered = false) {
    this.hoveredBar = hovered ? { sheetId, swapHovered } : null;
    const node = this.sheetNodes.get(sheetId);
    if (node) setSheetBarOverlayHovered(node.sheetBar, hovered, swapHovered);
  }

  handleSheetBarSwapFocus(sheetId: string, focused: boolean) {
    this.focusedBarSheetId = focused ? sheetId : null;
    const node = this.sheetNodes.get(sheetId);
    if (node) setSheetBarSwapFocused(node.sheetBar, focused);
  }

  update(input: AlbumCanvasProps, hostHeight: number) {
    const projectChanged = this.projectId !== input.projectId;
    if (projectChanged) {
      this.resetProjectScene();
      this.projectId = input.projectId;
      this.projectGeneration += 1;
    }
    const returnedToContinuousCanvas =
      !projectChanged &&
      this.input?.mode.kind === "sheet-editing" &&
      input.mode.kind === "normal";
    const modeSignature = JSON.stringify(input.mode);
    if (
      this.modeSignature !== null &&
      this.modeSignature !== modeSignature
    ) {
      this.resetTransientInteractions();
      this.lastCanvasMetrics = null;
    }
    this.modeSignature = modeSignature;
    this.input = input;
    this.app.canvas.setAttribute("aria-label", input.mode.kind === "sheet-editing"
      ? "Canvas da Lâmina em edição. Arraste um Frame para mover ou use as alças para redimensionar. Shift preserva a proporção; Alt preserva o centro; Esc cancela o gesto."
      : "Canvas contínuo do Álbum. Arraste uma Foto sobre outro Frame para trocar o conteúdo, inclusive entre Lâminas. Esc cancela. Use a roda para navegar, Alt mais arraste para Pan e Alt mais roda para Zoom.");
    const modePolicy = albumCanvasModePolicy(input.mode);
    const confirmedSheets = sheetsForCanvasMode(
      input.composition.sheets,
      modePolicy,
    );
    const reorderPreview = resolveBarSheetReorderPreview(
      input,
      confirmedSheets,
    );
    const shouldAnimateSheetPositions =
      reorderPreview !== null || this.sheetReorderPreviewActive;
    this.sheetReorderPreviewActive = reorderPreview !== null;
    this.sheetReorderPlaceholderSheetId =
      reorderPreview?.draggedSheetId ?? null;
    const sheets = reorderPreview?.sheets ?? confirmedSheets;
    const firstSheet = sheets[0];
    if (!firstSheet) {
      this.clearMaterializedSheets();
      this.previewTextures.sync([]);
      return;
    }
    const navigationLayout = input.continuousCanvasLayout;
    const layout = !modePolicy.enablesContinuousNavigation
      ? createContinuousCanvasLayout(sheets)
      : reorderPreview
        ? createReorderedCanvasLayout(input, sheets)
        : navigationLayout;

    const sheetHeight = firstSheet.heightUm * MICROMETER_TO_CANVAS_PIXEL;
    const heightScale = continuousCanvasScale(
      hostHeight || this.app.screen.height,
      sheetHeight,
    );
    const scale = modePolicy.enablesContinuousNavigation ? heightScale : Math.min(
      heightScale,
      Math.max(1, this.app.screen.width - 2 * CANVAS_VERTICAL_MARGIN_PX) /
        layout.entriesAtScale(1)[0].width,
    );
    this.canvasScale = scale;
    this.frameInteractions.synchronize(input, scale);
    this.frameContentDrag.synchronize(input);
    const transitionOffsetX =
      returnedToContinuousCanvas && input.centeredSheetId
        ? navigationLayout.centeredOffset(
            input.centeredSheetId,
            scale,
            this.app.screen.width,
          )
        : null;
    if (transitionOffsetX !== null) {
      this.pendingViewportOffsetX = transitionOffsetX;
    } else if (
      this.pendingViewportOffsetX !== null &&
      Math.abs(input.viewport.offsetX - this.pendingViewportOffsetX) < 0.0001
    ) {
      this.pendingViewportOffsetX = null;
    }
    const requestedOffsetX =
      transitionOffsetX ??
      this.pendingViewportOffsetX ??
      input.viewport.offsetX;
    const boundedOffsetX =
      modePolicy.enablesContinuousNavigation
        ? navigationLayout.clampOffset(
            requestedOffsetX,
            scale,
            this.app.screen.width,
          )
        : (layout.centeredOffset(
            modePolicy.editingSheetId,
            scale,
            this.app.screen.width,
          ) ?? 0);
    if (
      modePolicy.enablesContinuousNavigation &&
      Math.abs(boundedOffsetX - input.viewport.offsetX) > 0.0001
    ) {
      input.onViewportChange({
        ...input.viewport,
        offsetX: boundedOffsetX,
      });
    }

    if (modePolicy.enablesContinuousNavigation) {
      this.synchronizeCenteredSheet(
        navigationLayout,
        boundedOffsetX,
        scale,
      );
    }
    this.reportCanvasMetrics(scale);
    this.world.position.set(
      boundedOffsetX,
      modePolicy.enablesContinuousNavigation ? CANVAS_VERTICAL_MARGIN_PX
        : ((hostHeight || this.app.screen.height) - sheetHeight * scale) / 2,
    );
    this.world.scale.set(scale);
    this.app.stage.hitArea = new Rectangle(
      0,
      0,
      this.app.screen.width,
      this.app.screen.height,
    );

    const presentedSheets = this.frameInteractions.present(sheets);
    this.reconcileMaterializedSheets(
      presentedSheets,
      layout,
      boundedOffsetX,
      scale,
      shouldAnimateSheetPositions,
    );
    this.updateDecorations(presentedSheets);
    this.photoInteractions.applyExternalPreview();
  }

  resize(hostHeight: number) {
    this.app.resize();
    if (this.input) this.update(this.input, hostHeight);
  }

  destroy() {
    this.resetTransientInteractions();
    this.frameInteractions.destroy();
    this.frameContentDrag.destroy();
    this.frameContentDragVisual.destroy();
    this.app.stage.removeAllListeners();
    this.clearMaterializedSheets();
    this.previewTextures.destroy();
  }

  suspendForContextLoss() {
    this.resetTransientInteractions();
    this.input?.onTransformPreview(null);
  }

  resolveSheetAtPoint(clientX: number, clientY: number): string | null {
    const worldPoint = this.resolveWorldPoint(clientX, clientY);
    if (!worldPoint) return null;
    for (const [sheetId, node] of this.sheetNodes) {
      const localX = worldPoint.x - node.container.position.x;
      const localY = worldPoint.y - node.container.position.y;
      const { x, y, width, height } = node.viewBounds;
      if (
        localX >= x &&
        localY >= y &&
        localX < x + width &&
        localY < y + height
      ) {
        return sheetId;
      }
    }
    return null;
  }

  resolvePhotoDropPoint(
    clientX: number,
    clientY: number,
  ): { sheetId: string; xUm: number; yUm: number } | null {
    const worldPoint = this.resolveWorldPoint(clientX, clientY);
    if (!worldPoint || !this.input) return null;
    for (const [sheetId, node] of this.sheetNodes) {
      const sheet = this.input.composition.sheets.find(
        (candidate) => candidate.sheetId === sheetId,
      );
      if (!sheet) continue;
      const localX =
        worldPoint.x - node.container.position.x - node.activeOffsetXPx;
      const localY = worldPoint.y - node.container.position.y;
      const width = sheet.widthUm * MICROMETER_TO_CANVAS_PIXEL;
      const height = sheet.heightUm * MICROMETER_TO_CANVAS_PIXEL;
      if (localX < 0 || localY < 0 || localX >= width || localY >= height) {
        continue;
      }
      return {
        sheetId,
        xUm: Math.floor(localX / MICROMETER_TO_CANVAS_PIXEL),
        yUm: Math.floor(localY / MICROMETER_TO_CANVAS_PIXEL),
      };
    }
    return null;
  }

  private resolveWorldPoint(
    clientX: number,
    clientY: number,
  ): { x: number; y: number } | null {
    if (!this.input || this.canvasScale <= 0) return null;
    const bounds = this.app.canvas.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) return null;
    const canvasX =
      (clientX - bounds.left) * (this.app.screen.width / bounds.width);
    const canvasY =
      (clientY - bounds.top) * (this.app.screen.height / bounds.height);
    return {
      x: (canvasX - this.world.position.x) / this.canvasScale,
      y: (canvasY - this.world.position.y) / this.canvasScale,
    };
  }

  private resetProjectScene() {
    this.resetTransientInteractions();
    this.clearMaterializedSheets();
    this.previewTextures.sync([]);
    this.lastCanvasMetrics = null;
    this.lastMediaDemandSignature = null;
    this.modeSignature = null;
    this.pendingViewportOffsetX = null;
  }

  private resetTransientInteractions() {
    this.hoveredBar = null;
    this.focusedBarSheetId = null;
    this.photoInteractions.reset();
    this.frameInteractions.reset();
    this.frameContentDrag.reset();
    this.frameContentDragVisual.reset();
    delete this.app.canvas.dataset.frameContentDragTarget;
    for (const node of this.sheetNodes.values()) {
      for (const highlight of node.frameContentDropHighlights.values()) highlight.visible = false;
    }
    this.sheetReorderPreviewActive = false;
    this.sheetReorderPlaceholderSheetId = null;
    this.stopSheetPositionAnimations();
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
    if (centeredSheetId && centeredSheetId !== this.input.centeredSheetId) {
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
    sheets: readonly ComposedSheet[],
    layout: ContinuousCanvasLayout,
    boundedOffsetX: number,
    scale: number,
    animateSheetPositions: boolean,
  ) {
    if (!this.input) return;
    const entries = layout.entriesAtScale(scale);
    const viewportLeft = -boundedOffsetX / scale;
    const viewportRight = viewportLeft + this.app.screen.width / scale;
    const visibleIndexes = entries
      .filter(
        ({ left, right }) =>
          right >= viewportLeft && left <= viewportRight,
      )
      .map(({ index }) => index);
    const residentIndexes = entries
      .filter(
        ({ left, right }) =>
          right >= viewportLeft - VIEWPORT_PRELOAD_PX &&
          left <= viewportRight + VIEWPORT_PRELOAD_PX,
      )
      .map(({ index }) => index);
    const firstVisible = Math.max(0, (residentIndexes[0] ?? 0) - PRELOAD_MARGIN);
    const lastVisible = Math.min(
      sheets.length - 1,
      (residentIndexes[residentIndexes.length - 1] ?? 0) + PRELOAD_MARGIN,
    );
    const desiredSheets = sheets.slice(firstVisible, lastVisible + 1);
    this.reportMediaDemand(
      visibleIndexes.map((index) => sheets[index]),
      desiredSheets,
    );
    const desiredIds = new Set(desiredSheets.map((sheet) => sheet.sheetId));
    const desiredPreviewUrls = new Set<string>();
    const signatures = new Map<string, string>();
    const sheetBarMetadata = new Map(
      this.input.sheetBarMetadata.map((metadata) => [
        metadata.sheetId,
        metadata,
      ]),
    );

    for (const sheet of desiredSheets) {
      const previewStates = sheet.frames.map((frame) => {
        if (!frame.photo) return null;
        const url = this.input?.mediaPreviewUrls?.[frame.photo.mediaId] ?? null;
        if (url) desiredPreviewUrls.add(url);
        return url ? [url, this.previewTextures.get(url) !== undefined] : null;
      });
      const backgroundPreviewStates = sheet.backgrounds.flatMap(
        (background) => {
          if (background.kind !== "media") return [];
          const url =
            this.input?.mediaPreviewUrls?.[background.mediaId] ?? null;
          if (url) desiredPreviewUrls.add(url);
          return [
            url
              ? [url, this.previewTextures.get(url) !== undefined]
              : null,
          ];
        },
      );
      const overlayPreviewStates = sheet.overlays.map((overlay) => {
        const url =
          this.input?.mediaPreviewUrls?.[overlay.mediaId] ?? null;
        if (url) desiredPreviewUrls.add(url);
        return url
          ? [url, this.previewTextures.get(url) !== undefined]
          : null;
      });
      signatures.set(
        sheet.sheetId,
        JSON.stringify([
          sheet,
          sheetBarMetadata.get(sheet.sheetId) ?? null,
          this.input.technicalGuides ?? null,
          albumCanvasModePolicy(this.input.mode),
          previewStates,
          backgroundPreviewStates,
          overlayPreviewStates,
        ]),
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
      const created = !node;
      if (!node) {
        node = this.createSheetNode(
          sheet,
          sheetBarMetadata.get(sheet.sheetId),
          signature,
        );
        this.sheetNodes.set(sheet.sheetId, node);
        this.world.addChild(node.container);
      }
      applySheetBarScale(node.sheetBar, scale);
      if (this.hoveredBar?.sheetId === sheet.sheetId) {
        setSheetBarOverlayHovered(node.sheetBar, true, this.hoveredBar.swapHovered);
      }
      if (this.focusedBarSheetId === sheet.sheetId) {
        setSheetBarSwapFocused(node.sheetBar, true);
      }
      applyPlaceholderLabelScale(node, scale);
      for (const selection of node.frameSelections.values()) {
        applyFrameSelectionScale(selection, scale);
      }
      this.moveSheetNode(
        sheet.sheetId,
        node,
        entries[index].left - node.viewBounds.x,
        created ? false : animateSheetPositions,
      );
    }
  }

  private moveSheetNode(
    sheetId: string,
    node: SheetRenderNode,
    targetX: number,
    animate: boolean,
  ) {
    node.container.position.y = 0;
    const currentAnimation = this.sheetPositionAnimations.get(sheetId);
    if (!animate) {
      if (currentAnimation?.toX === targetX) return;
      this.sheetPositionAnimations.delete(sheetId);
      node.container.position.x = targetX;
      this.detachSheetPositionTickerIfIdle();
      return;
    }
    if (currentAnimation?.toX === targetX) return;
    if (Math.abs(node.container.position.x - targetX) < 0.0001) {
      this.sheetPositionAnimations.delete(sheetId);
      this.detachSheetPositionTickerIfIdle();
      return;
    }
    this.sheetPositionAnimations.set(sheetId, {
      fromX: node.container.position.x,
      toX: targetX,
      elapsedMs: 0,
    });
    if (!this.sheetPositionTickerAttached) {
      this.sheetPositionTickerAttached = true;
      this.app.ticker.add(this.advanceSheetPositionAnimations);
    }
  }

  private readonly advanceSheetPositionAnimations = (ticker: Ticker) => {
    for (const [sheetId, animation] of this.sheetPositionAnimations) {
      const node = this.sheetNodes.get(sheetId);
      if (!node) {
        this.sheetPositionAnimations.delete(sheetId);
        continue;
      }
      animation.elapsedMs = Math.min(
        SHEET_REORDER_TRANSITION_MS,
        animation.elapsedMs + Math.max(0, ticker.deltaMS),
      );
      const progress = animation.elapsedMs / SHEET_REORDER_TRANSITION_MS;
      const easedProgress = 1 - (1 - progress) ** 3;
      node.container.position.x =
        animation.fromX +
        (animation.toX - animation.fromX) * easedProgress;
      if (progress >= 1) {
        node.container.position.x = animation.toX;
        this.sheetPositionAnimations.delete(sheetId);
      }
    }
    this.detachSheetPositionTickerIfIdle();
  };

  private detachSheetPositionTickerIfIdle() {
    if (
      !this.sheetPositionTickerAttached ||
      this.sheetPositionAnimations.size > 0
    ) {
      return;
    }
    this.app.ticker.remove(this.advanceSheetPositionAnimations);
    this.sheetPositionTickerAttached = false;
  }

  private stopSheetPositionAnimations() {
    this.sheetPositionAnimations.clear();
    this.detachSheetPositionTickerIfIdle();
  }

  private reportMediaDemand(
    visibleSheets: readonly ComposedSheet[],
    residentSheets: readonly ComposedSheet[],
  ) {
    if (!this.input?.onMediaDemandChange) return;
    const visibleMediaIds = mediaIdsForSheets(visibleSheets);
    const visible = new Set(visibleMediaIds);
    const preloadMediaIds = mediaIdsForSheets(residentSheets).filter(
      (mediaId) => !visible.has(mediaId),
    );
    const signature = JSON.stringify([visibleMediaIds, preloadMediaIds]);
    if (signature === this.lastMediaDemandSignature) return;
    this.lastMediaDemandSignature = signature;
    this.input.onMediaDemandChange({
      visibleMediaIds,
      preloadMediaIds,
    });
  }

  private clearMaterializedSheets() {
    for (const [sheetId, node] of this.sheetNodes) {
      this.removeSheetNode(sheetId, node);
    }
  }

  private removeSheetNode(sheetId: string, node: SheetRenderNode) {
    this.sheetPositionAnimations.delete(sheetId);
    this.detachSheetPositionTickerIfIdle();
    this.world.removeChild(node.container);
    for (const photoNode of node.photoNodes) {
      this.photoNodes.delete(photoNode.frameId);
    }
    destroySheetRenderNode(node);
    this.sheetNodes.delete(sheetId);
  }

  private createSheetNode(
    sheet: ComposedSheet,
    sheetBarMetadata: SheetBarMetadata | undefined,
    signature: string,
  ): SheetRenderNode {
    const node = createSheetRenderNode(
      sheet,
      sheetBarMetadata,
      this.input?.technicalGuides,
      albumCanvasModePolicy(
        this.input?.mode ?? { kind: "normal" },
      ),
      signature,
      {
        previewTextureFor: (mediaId) => this.previewTextureFor(mediaId),
        onSheetTap: (sheetId) => {
          if (!this.input || this.frameInteractions.ignoresTap || this.frameContentDrag.ignoresTap) return;
          this.input.onSelectFrame(null);
          this.input.onFocusSheet(sheetId);
        },
        onSheetDoubleTap: (sheetId) => {
          if (this.frameInteractions.ignoresTap || this.frameContentDrag.ignoresTap) return;
          this.input?.onEditSheet(sheetId);
        },
        onFrameTap: (sheetId, frameId, toggle) => {
          if (!this.input || this.frameInteractions.ignoresTap || this.frameContentDrag.ignoresTap) return;
          if (toggle) this.input.onSelectFrame(frameId, true);
          else this.input.onSelectFrame(frameId);
          this.input.onFocusSheet(sheetId);
        },
        onPhotoPanStart: (photoNode, event) => {
          this.photoInteractions.startPan(photoNode, event);
        },
        onPhotoContentDragStart: (frameId, event) => this.frameContentDrag.start(frameId, event),
        onFrameContextMenu: (frameId, position) => {
          if (!this.input || this.input.frameGeometry?.disabled || this.frameInteractions.ignoresTap) return;
          this.input.onOpenFrameContextMenu?.(frameId, position);
        },
        onEmptyCanvasContextMenu: (sheetId, position) => this.openEmptyCanvasContextMenu(sheetId, position),
        onFrameGeometryStart: (frameId, handle, event) => {
          this.frameInteractions.start(frameId, handle, event);
        },
        onPhotoWheel: (photoNode, event) => {
          this.photoInteractions.handleWheel(photoNode, event);
        },
      },
    );
    for (const photoNode of node.photoNodes) {
      this.photoNodes.set(photoNode.frameId, photoNode);
    }
    return node;
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

  private updateDecorations(sheets: readonly ComposedSheet[]) {
    if (!this.input) return;
    this.frameContentDragVisual.update(this.frameContentDrag.preview);
    if (this.frameContentDrag.preview) {
      this.app.canvas.dataset.frameContentDragTarget = this.frameContentDrag.highlight?.kind === "frame"
        ? this.frameContentDrag.highlight.frameId : "";
    } else delete this.app.canvas.dataset.frameContentDragTarget;
    const highlight = this.input.photoDropHighlight;
    for (const [sheetId, node] of this.sheetNodes) {
      node.container.visible =
        sheetId !== this.sheetReorderPlaceholderSheetId;
      node.focusOutline.visible = sheetId === this.input.focusedSheetId;
      node.sheetDropOutline.visible =
        this.input.photoDropHighlight?.kind === "sheet" &&
        this.input.photoDropHighlight.sheetId === sheetId;
      for (const [frameId, selection] of node.frameSelections) {
        selection.container.visible = this.input.selectedFrameIds.includes(frameId);
        for (const handle of selection.resizeHandles) {
          handle.visible = this.input.selectedFrameIds.length === 1;
        }
      }
      this.updateFrameGroupSelection(node, sheets.find((sheet) => sheet.sheetId === sheetId));
      for (const [frameId, outline] of node.frameDropOutlines) {
        outline.visible =
          highlight?.kind === "frame" && highlight.frameId === frameId;
      }
      for (const [frameId, highlight] of node.frameContentDropHighlights) {
        highlight.visible = this.frameContentDrag.highlight?.kind === "frame" &&
          this.frameContentDrag.highlight.frameId === frameId;
      }
    }
  }

  private openEmptyCanvasContextMenu(sheetId: string, position: { x: number; y: number }) {
    if (!this.input || this.input.frameGeometry?.disabled || this.frameInteractions.ignoresTap ||
        this.input.sheetBarMetadata.find((sheet) => sheet.sheetId === sheetId)?.layoutLocked) return;
    this.input.onOpenEmptyCanvasContextMenu?.(sheetId, position);
  }

  private updateFrameGroupSelection(node: SheetRenderNode, sheet: ComposedSheet | undefined) {
    if (!this.input || !sheet) return;
    const frames = sheet.frames.filter((frame) => this.input!.selectedFrameIds.includes(frame.frameId));
    const showHandles = this.input.mode.kind === "sheet-editing" &&
      !this.input.sheetBarMetadata.find((item) => item.sheetId === sheet.sheetId)?.layoutLocked;
    const signature = frames.length > 1
      ? JSON.stringify([frames.map((frame) => [frame.frameId, frame.clipRect]), showHandles]) : null;
    if (node.frameGroupSelection?.signature !== signature) {
      if (node.frameGroupSelection) {
        node.frameSelectionLayer.removeChild(node.frameGroupSelection.node.container);
        node.frameGroupSelection.node.container.destroy({ children: true });
        node.frameGroupSelection = null;
      }
      if (signature) {
        const left = Math.min(...frames.map((frame) => frame.clipRect.x));
        const top = Math.min(...frames.map((frame) => frame.clipRect.y));
        const right = Math.max(...frames.map((frame) => frame.clipRect.x + frame.clipRect.width));
        const bottom = Math.max(...frames.map((frame) => frame.clipRect.y + frame.clipRect.height));
        const selection = createFrameSelectionRenderNode(
          `group-${sheet.sheetId}`, (right - left) * MICROMETER_TO_CANVAS_PIXEL,
          (bottom - top) * MICROMETER_TO_CANVAS_PIXEL, showHandles,
          (handle, event) => this.frameInteractions.start(frames[0].frameId, handle, event),
        );
        selection.container.position.set(left * MICROMETER_TO_CANVAS_PIXEL, top * MICROMETER_TO_CANVAS_PIXEL);
        selection.container.visible = true;
        node.frameSelectionLayer.addChild(selection.container);
        node.frameGroupSelection = { signature, node: selection };
      }
    }
    if (node.frameGroupSelection) applyFrameSelectionScale(node.frameGroupSelection.node, this.canvasScale);
  }

  readonly handleCanvasWheel = (event: WheelEvent) => {
    if (
      event.defaultPrevented ||
      !this.input ||
      !albumCanvasModePolicy(this.input.mode).enablesContinuousNavigation ||
      event.altKey
    ) {
      return;
    }
    event.preventDefault();
    if (event.ctrlKey) return;
    this.scrollContinuousCanvas((event.deltaX || event.deltaY) * 0.9);
  };

  private scrollContinuousCanvas(deltaPx: number) {
    if (!this.input || this.input.mode.kind !== "normal") return;
    const layout = this.input.continuousCanvasLayout;
    this.pendingViewportOffsetX = null;
    const nextOffset = layout.clampOffset(
      this.input.viewport.offsetX - deltaPx,
      this.canvasScale,
      this.app.screen.width,
    );
    this.input.onViewportChange({
      ...this.input.viewport,
      offsetX: nextOffset,
    });
    this.synchronizeCenteredSheet(layout, nextOffset, this.canvasScale);
  }
}

function resolveBarSheetReorderPreview(
  input: AlbumCanvasProps,
  confirmedSheets: readonly ComposedSheet[],
): BarSheetReorderPreview | null {
  const reorder = input.sheetReorder;
  if (
    input.mode.kind !== "normal" ||
    (reorder?.status !== "preview" && reorder?.status !== "committing") ||
    reorder.representation.ghost === null ||
    reorder.representation.placeholderIndex === null ||
    reorder.representation.order.length !== confirmedSheets.length
  ) {
    return null;
  }
  const sheetsById = new Map(
    confirmedSheets.map((sheet) => [sheet.sheetId, sheet]),
  );
  const reorderedSheets: ComposedSheet[] = [];
  const seenSheetIds = new Set<string>();
  for (const sheetId of reorder.representation.order) {
    const sheet = sheetsById.get(sheetId);
    if (!sheet || seenSheetIds.has(sheetId)) return null;
    seenSheetIds.add(sheetId);
    reorderedSheets.push(sheet);
  }
  if (!sheetsById.has(reorder.representation.ghost.sheetId)) return null;
  return {
    draggedSheetId: reorder.representation.ghost.sheetId,
    sheets: reorderedSheets,
  };
}

function createReorderedCanvasLayout(
  input: AlbumCanvasProps,
  reorderedSheets: readonly ComposedSheet[],
) {
  const confirmedWidths = new Map(
    input.continuousCanvasLayout
      .entriesAtScale(1)
      .map((entry) => [entry.sheetId, entry.width]),
  );
  return createContinuousCanvasLayout(
    reorderedSheets,
    (sheet, presentation) =>
      confirmedWidths.get(sheet.sheetId) ?? presentation.visualWidthPx,
  );
}

function mediaIdsForSheets(sheets: readonly ComposedSheet[]) {
  const mediaIds = new Set<string>();
  for (const sheet of sheets) {
    for (const frame of sheet.frames) {
      if (frame.photo) mediaIds.add(frame.photo.mediaId);
    }
    for (const background of sheet.backgrounds) {
      if (background.kind === "media") mediaIds.add(background.mediaId);
    }
    for (const overlay of sheet.overlays) {
      mediaIds.add(overlay.mediaId);
    }
  }
  return [...mediaIds];
}
