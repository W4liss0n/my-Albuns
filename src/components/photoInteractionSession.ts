import { Container, FederatedPointerEvent, FederatedWheelEvent } from "pixi.js";

import type { NormalizedPan } from "../domain/project";
import type {
  AlbumCanvasProps,
  PhotoTransformDelta,
  PhotoTransformPreview,
} from "./albumCanvasContract";
import {
  applyPhotoPlacementPreview,
  applyPhotoZoomPreview,
  type PhotoRenderNode,
  resetPhotoPreview,
  setPhotoPanAids,
  setPhotoPreviewPosition,
} from "./albumCanvasRenderNodes";
import {
  advancePhotoZoomGesture,
  finishPhotoZoomGesture,
  type PhotoZoomGesture,
} from "./photoZoomGesture";

const ZOOM_GESTURE_SETTLE_MS = 500;

interface PhotoInteractionContext {
  input: Pick<
    AlbumCanvasProps,
    "photoZoomPreview" | "onTransformPreview" | "onTransformCommit"
  > | null;
  projectGeneration: number;
  canvasScale: number;
}

interface PanGesture {
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

/**
 * Owns one continuous photo interaction from live preview through its
 * eventual commit or cancellation. Rendering and sheet materialization stay
 * with AlbumCanvasScene; this module only coordinates photo gesture state.
 */
export class PhotoInteractionSession {
  private pan: PanGesture | null = null;
  private zoom: ZoomGestureRuntime | null = null;
  private readonly pendingCommitFrames = new Set<string>();
  private externalPreviewFrameId: string | null = null;

  constructor(
    private readonly photoNodes: ReadonlyMap<string, PhotoRenderNode>,
    private readonly readContext: () => PhotoInteractionContext,
  ) {}

  reset() {
    if (this.zoom?.timer != null) {
      window.clearTimeout(this.zoom.timer);
    }
    if (this.pan) {
      setPhotoPanAids(this.pan.node, false);
      resetPhotoPreview(this.pan.node);
    }
    if (this.zoom) {
      const zoomNode = this.photoNodes.get(this.zoom.gesture.frameId);
      if (zoomNode && zoomNode !== this.pan?.node) {
        resetPhotoPreview(zoomNode);
      }
    }
    if (this.externalPreviewFrameId) {
      const externalNode = this.photoNodes.get(this.externalPreviewFrameId);
      if (externalNode) resetPhotoPreview(externalNode);
    }
    this.zoom = null;
    this.pan = null;
    this.externalPreviewFrameId = null;
    this.pendingCommitFrames.clear();
  }

  startPan(
    frameContainer: Container,
    photoNode: PhotoRenderNode,
    event: FederatedPointerEvent,
  ) {
    if (this.pendingCommitFrames.has(photoNode.frameId)) return;
    const context = this.readContext();
    const activeZoom = this.zoom;
    const continuesZoom = activeZoom?.gesture.frameId === photoNode.frameId;
    if (continuesZoom && activeZoom.timer !== null) {
      window.clearTimeout(activeZoom.timer);
      activeZoom.timer = null;
    }
    this.pan = {
      generation: context.projectGeneration,
      frameId: photoNode.frameId,
      startX: event.global.x,
      startY: event.global.y,
      canvasScale: context.canvasScale,
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

  handleWheel(photoNode: PhotoRenderNode, event: FederatedWheelEvent) {
    const context = this.readContext();
    if (!context.input) return;
    event.preventDefault();
    if (this.pendingCommitFrames.has(photoNode.frameId)) return;
    const current = this.zoom;
    if (current?.timer != null) {
      window.clearTimeout(current.timer);
    }
    const transition = advancePhotoZoomGesture(current?.gesture ?? null, {
      frameId: photoNode.frameId,
      baseZoom: photoNode.baseZoom,
      zoomRange: photoNode.geometry.zoomRange,
      wheelDeltaY: event.deltaY,
    });
    if (transition.interruptedCommit) {
      const interruptedNode = this.photoNodes.get(
        transition.interruptedCommit.frameId,
      );
      if (interruptedNode) {
        this.commit(
          {
            frameId: transition.interruptedCommit.frameId,
            deltaPanX: 0,
            deltaPanY: 0,
            deltaZoom: transition.interruptedCommit.delta,
          },
          interruptedNode,
          current?.generation ?? context.projectGeneration,
        );
      }
    }

    const activePan = this.pan;
    if (activePan?.frameId === photoNode.frameId) {
      const combined = photoNode.geometry.constrain(
        {
          x: activePan.currentX,
          y: activePan.currentY,
        },
        transition.previewZoom,
      );
      activePan.currentX = combined.placement.center.x;
      activePan.currentY = combined.placement.center.y;
      activePan.currentPan = combined.pan;
      activePan.currentZoom = combined.zoom;
      applyPhotoPlacementPreview(photoNode, combined.zoom, combined.placement);
      context.input.onTransformPreview(
        createTransformPreview(
          activePan.frameId,
          activePan.currentPan,
          activePan.currentZoom,
        ),
      );
      this.zoom = {
        generation: context.projectGeneration,
        gesture: transition.gesture,
        timer: null,
      };
      return;
    }

    applyPhotoZoomPreview(photoNode, transition.previewZoom);
    context.input.onTransformPreview(
      createTransformPreview(
        photoNode.frameId,
        photoNode.pan,
        transition.previewZoom,
      ),
    );
    const runtime: ZoomGestureRuntime = {
      generation: context.projectGeneration,
      gesture: transition.gesture,
      timer: null,
    };
    runtime.timer = window.setTimeout(() => {
      const currentContext = this.readContext();
      if (
        this.zoom !== runtime ||
        runtime.generation !== currentContext.projectGeneration
      ) {
        return;
      }
      this.zoom = null;
      const commit = finishPhotoZoomGesture(runtime.gesture);
      const commitNode = commit ? this.photoNodes.get(commit.frameId) : null;
      if (commit && commitNode) {
        this.commit(
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
        currentContext.input?.onTransformPreview(null);
      }
    }, ZOOM_GESTURE_SETTLE_MS);
    this.zoom = runtime;
  }

  readonly handlePointerMove = (event: FederatedPointerEvent) => {
    const context = this.readContext();
    if (
      !this.pan ||
      !context.input ||
      this.pan.generation !== context.projectGeneration
    ) {
      return;
    }
    updatePanPreview(this.pan, event.global.x, event.global.y);
    context.input.onTransformPreview(
      createTransformPreview(
        this.pan.frameId,
        this.pan.currentPan,
        this.pan.currentZoom,
      ),
    );
  };

  readonly finishPan = (event: FederatedPointerEvent) => {
    const gesture = this.pan;
    const context = this.readContext();
    if (
      !gesture ||
      !context.input ||
      gesture.generation !== context.projectGeneration
    ) {
      return;
    }
    updatePanPreview(gesture, event.global.x, event.global.y);
    this.pan = null;
    setPhotoPanAids(gesture.node, false);

    const deltaPanX = gesture.currentPan.x - gesture.node.pan.x;
    const deltaPanY = gesture.currentPan.y - gesture.node.pan.y;
    const deltaZoom = gesture.currentZoom - gesture.node.baseZoom;
    const combinedZoom = this.zoom;
    const ownsCombinedZoom = combinedZoom?.gesture.frameId === gesture.frameId;
    const changedPan =
      Math.abs(deltaPanX) > 0.0001 || Math.abs(deltaPanY) > 0.0001;
    const changedZoom = Math.abs(deltaZoom) > 0.0001;

    if (ownsCombinedZoom) {
      if (combinedZoom.timer !== null) {
        window.clearTimeout(combinedZoom.timer);
      }
      this.zoom = null;
    }

    if ((ownsCombinedZoom && changedZoom) || changedPan) {
      context.input.onTransformPreview(
        createTransformPreview(
          gesture.frameId,
          gesture.currentPan,
          gesture.currentZoom,
        ),
      );
      this.commit(
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
      context.input.onTransformPreview(null);
    }
  };

  readonly cancelPan = () => {
    const gesture = this.pan;
    if (!gesture) return;
    this.pan = null;
    setPhotoPanAids(gesture.node, false);
    resetPhotoPreview(gesture.node);
    this.readContext().input?.onTransformPreview(null);

    const combinedZoom = this.zoom;
    if (combinedZoom?.gesture.frameId === gesture.frameId) {
      if (combinedZoom.timer !== null) {
        window.clearTimeout(combinedZoom.timer);
      }
      this.zoom = null;
    }
  };

  applyExternalPreview() {
    const input = this.readContext().input;
    if (!input) return;
    const previousFrameId = this.externalPreviewFrameId;
    const nextPreview = input.photoZoomPreview;
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

  private commit(
    delta: PhotoTransformDelta,
    node: PhotoRenderNode,
    generation: number,
  ) {
    const context = this.readContext();
    if (
      !context.input ||
      generation !== context.projectGeneration ||
      this.pendingCommitFrames.has(delta.frameId)
    ) {
      return;
    }

    this.pendingCommitFrames.add(delta.frameId);
    let result: Promise<boolean>;
    try {
      result = context.input.onTransformCommit(delta);
    } catch {
      this.settle(delta.frameId, node, generation, false);
      return;
    }
    void result.then(
      (accepted) => this.settle(delta.frameId, node, generation, accepted),
      () => this.settle(delta.frameId, node, generation, false),
    );
  }

  private settle(
    frameId: string,
    node: PhotoRenderNode,
    generation: number,
    accepted: boolean,
  ) {
    const context = this.readContext();
    if (generation !== context.projectGeneration) return;
    this.pendingCommitFrames.delete(frameId);
    if (!accepted && this.photoNodes.get(frameId) === node) {
      resetPhotoPreview(node);
      context.input?.onTransformPreview(null);
    }
  }
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

function updatePanPreview(
  gesture: PanGesture,
  pointerX: number,
  pointerY: number,
) {
  const nextX =
    gesture.originalX + (pointerX - gesture.startX) / gesture.canvasScale;
  const nextY =
    gesture.originalY + (pointerY - gesture.startY) / gesture.canvasScale;
  const constrained = gesture.node.geometry.constrain(
    { x: nextX, y: nextY },
    gesture.currentZoom,
  );
  gesture.currentX = constrained.placement.center.x;
  gesture.currentY = constrained.placement.center.y;
  gesture.currentPan = constrained.pan;
  setPhotoPreviewPosition(gesture.node, gesture.currentX, gesture.currentY);
}
