import { Application, Container, Rectangle } from "pixi.js";

import type { ComposedSheet } from "../domain/project";
import type {
  AlbumCanvasProps,
  CanvasMetrics,
  SheetBarMetadata,
} from "./albumCanvasContract";
import {
  CANVAS_VERTICAL_MARGIN_PX,
  continuousCanvasScale,
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
import { applySheetBarScale } from "./sheetBarRenderNode";
import { PhotoInteractionSession } from "./photoInteractionSession";
import { ViewportTexturePool } from "./viewportTexturePool";

const PRELOAD_MARGIN = 1;
const VIEWPORT_PRELOAD_PX = 700;

export class AlbumCanvasScene {
  private readonly world = new Container();
  private readonly sheetNodes = new Map<string, SheetRenderNode>();
  private readonly photoNodes = new Map<string, PhotoRenderNode>();
  private input: AlbumCanvasProps | null = null;
  private projectId: string | null = null;
  private projectGeneration = 0;
  private canvasScale = 1;
  private lastCanvasMetrics: CanvasMetrics | null = null;
  private lastMediaDemandSignature: string | null = null;
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

    const sheetHeight = firstSheet.heightUm * MICROMETER_TO_CANVAS_PIXEL;
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
      CANVAS_VERTICAL_MARGIN_PX,
    );
    this.world.scale.set(scale);
    this.app.stage.hitArea = new Rectangle(
      0,
      0,
      this.app.screen.width,
      this.app.screen.height,
    );

    this.reconcileMaterializedSheets(layout, boundedOffsetX, scale);
    this.updateDecorations();
    this.photoInteractions.applyExternalPreview();
  }

  resize(hostHeight: number) {
    this.app.resize();
    if (this.input) this.update(this.input, hostHeight);
  }

  destroy() {
    this.resetTransientInteractions();
    this.app.canvas.removeEventListener("wheel", this.handleCanvasWheel);
    this.app.stage.removeAllListeners();
    this.clearMaterializedSheets();
    this.previewTextures.destroy();
  }

  suspendForContextLoss() {
    this.resetTransientInteractions();
    this.input?.onTransformPreview(null);
  }

  private resetProjectScene() {
    this.resetTransientInteractions();
    this.clearMaterializedSheets();
    this.previewTextures.sync([]);
    this.lastCanvasMetrics = null;
    this.lastMediaDemandSignature = null;
  }

  private resetTransientInteractions() {
    this.photoInteractions.reset();
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
    layout: ContinuousCanvasLayout,
    boundedOffsetX: number,
    scale: number,
  ) {
    if (!this.input) return;
    const sheets = this.input.composition.sheets;
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
      node.container.position.set(entries[index].left, 0);
    }
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
      signature,
      {
        previewTextureFor: (mediaId) => this.previewTextureFor(mediaId),
        onSheetTap: (sheetId) => {
          if (!this.input) return;
          this.input.onSelectFrame(null);
          this.input.onFocusSheet(sheetId);
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
      node.focusOutline.visible = sheetId === this.input.focusedSheetId;
      for (const [frameId, outline] of node.selectionOutlines) {
        outline.visible = frameId === this.input.selectedFrameId;
      }
    }
  }

  private readonly handleCanvasWheel = (event: WheelEvent) => {
    if (!this.input || event.altKey) return;
    event.preventDefault();
    if (event.ctrlKey) return;
    const layout = this.input.continuousCanvasLayout;
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
