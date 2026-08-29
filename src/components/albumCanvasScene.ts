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
import { applyFrameSelectionScale } from "./frameSelectionRenderNode";
import {
  albumCanvasModePolicy,
  sheetsForCanvasMode,
} from "./albumCanvasMode";
import { applySheetBarScale } from "./sheetBarRenderNode";
import { PhotoInteractionSession } from "./photoInteractionSession";
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
    this.world.label = "album-world";
    this.app.stage.addChild(this.world);
    this.app.stage.eventMode = "static";
    this.app.stage.on(
      "globalpointermove",
      this.photoInteractions.handlePointerMove,
    );
    this.app.stage.on("pointerup", this.photoInteractions.finishPan);
    this.app.stage.on("pointerupoutside", this.photoInteractions.finishPan);
    this.app.stage.on("pointercancel", this.photoInteractions.cancelPan);
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
    const scale = continuousCanvasScale(
      hostHeight || this.app.screen.height,
      sheetHeight,
    );
    this.canvasScale = scale;
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
      CANVAS_VERTICAL_MARGIN_PX,
    );
    this.world.scale.set(scale);
    this.app.stage.hitArea = new Rectangle(
      0,
      0,
      this.app.screen.width,
      this.app.screen.height,
    );

    this.reconcileMaterializedSheets(
      sheets,
      layout,
      boundedOffsetX,
      scale,
      shouldAnimateSheetPositions,
    );
    this.updateDecorations();
    this.photoInteractions.applyExternalPreview();
  }

  resize(hostHeight: number) {
    this.app.resize();
    if (this.input) this.update(this.input, hostHeight);
  }

  destroy() {
    this.resetTransientInteractions();
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
    this.photoInteractions.reset();
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
          this.input.composition.frameBorder,
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
      this.input?.composition.frameBorder ?? { kind: "none" },
      this.input?.technicalGuides,
      albumCanvasModePolicy(
        this.input?.mode ?? { kind: "normal" },
      ),
      signature,
      {
        previewTextureFor: (mediaId) => this.previewTextureFor(mediaId),
        onSheetTap: (sheetId) => {
          if (!this.input) return;
          this.input.onSelectFrame(null);
          this.input.onFocusSheet(sheetId);
        },
        onSheetDoubleTap: (sheetId) => {
          this.input?.onEditSheet(sheetId);
        },
        onFrameTap: (sheetId, frameId) => {
          if (!this.input) return;
          this.input.onSelectFrame(frameId);
          this.input.onFocusSheet(sheetId);
        },
        onPhotoPanStart: (photoNode, event) => {
          this.photoInteractions.startPan(photoNode, event);
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

  private updateDecorations() {
    if (!this.input) return;
    for (const [sheetId, node] of this.sheetNodes) {
      node.container.visible =
        sheetId !== this.sheetReorderPlaceholderSheetId;
      node.focusOutline.visible = sheetId === this.input.focusedSheetId;
      node.sheetDropOutline.visible =
        this.input.photoDropHighlight?.kind === "sheet" &&
        this.input.photoDropHighlight.sheetId === sheetId;
      for (const [frameId, selection] of node.frameSelections) {
        selection.container.visible = frameId === this.input.selectedFrameId;
      }
      for (const [frameId, outline] of node.frameDropOutlines) {
        outline.visible =
          this.input.photoDropHighlight?.kind === "frame" &&
          this.input.photoDropHighlight.frameId === frameId;
      }
    }
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
    const layout = this.input.continuousCanvasLayout;
    this.pendingViewportOffsetX = null;
    const nextOffset = layout.clampOffset(
      this.input.viewport.offsetX - (event.deltaX || event.deltaY) * 0.9,
      this.canvasScale,
      this.app.screen.width,
    );
    this.input.onViewportChange({
      ...this.input.viewport,
      offsetX: nextOffset,
    });
    this.synchronizeCenteredSheet(layout, nextOffset, this.canvasScale);
  };
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
