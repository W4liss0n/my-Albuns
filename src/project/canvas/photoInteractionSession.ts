import { FederatedPointerEvent, FederatedWheelEvent } from "pixi.js";

import type { NormalizedPan } from "../../domain/project";
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
import type { ConstrainedPhotoPlacement } from "./photoGeometry";
import {
  advancePhotoZoomGesture,
  finishPhotoZoomGesture,
  type PhotoZoomGesture,
} from "./photoZoomGesture";

const ZOOM_GESTURE_SETTLE_MS = 500;

interface PhotoInteractionContext {
  input: Pick<
    AlbumCanvasProps,
    "photoZoomPreview" | "revision" | "onTransformPreview" | "onTransformCommit"
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
  // Where the pointer asks for the Photo center, before the Pan limit.
  targetX: number;
  targetY: number;
  currentX: number;
  currentY: number;
  currentPan: NormalizedPan;
  currentZoom: number;
}

interface ZoomGestureRuntime {
  generation: number;
  gesture: PhotoZoomGesture;
  node: PhotoRenderNode;
  timer: number | null;
}

/** A committed transform that the scene has not drawn from the Project yet. */
interface CommitPreview {
  node: PhotoRenderNode;
  shown: ConstrainedPhotoPlacement;
  /** The revision drawn when the Project accepted it; absent while it runs. */
  acceptedAt?: number;
}

/**
 * Owns one continuous photo interaction from live preview through its
 * eventual commit or cancellation. Rendering and sheet materialization stay
 * with AlbumCanvasScene; this module only coordinates photo gesture state.
 */
export class PhotoInteractionSession {
  private pan: PanGesture | null = null;
  private zoom: ZoomGestureRuntime | null = null;
  // Until the scene draws the committed projection, the Frame's node keeps
  // the placement from before the commit, so a new gesture would start there.
  private readonly commits = new Map<string, CommitPreview>();
  private externalPreviewFrameId: string | null = null;

  constructor(
    private readonly photoNodes: ReadonlyMap<string, PhotoRenderNode>,
    private readonly readContext: () => PhotoInteractionContext,
    // The settle timer and commit results change nodes outside any input event.
    private readonly requestRender: () => void = () => undefined,
  ) {}

  reset() {
    if (this.zoom?.timer != null) {
      window.clearTimeout(this.zoom.timer);
    }
    if (this.pan && !this.pan.node.layer.destroyed) {
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
    this.commits.clear();
  }

  /**
   * A mode change keeps what the user already let go of: a wheel Zoom waiting
   * to settle is committed, and running commits keep their previews on the
   * rebuilt nodes. An Alt-Pan still held is cancelled.
   */
  finishForModeChange() {
    const zoom = this.zoom;
    // A settling Zoom is never the one combined with the held Pan.
    if (zoom && zoom.timer !== null) {
      window.clearTimeout(zoom.timer);
      this.zoom = null;
      if (!this.commitZoom(zoom.gesture, zoom.generation)) {
        this.readContext().input?.onTransformPreview(null);
      }
    }
    this.cancelPan();
  }

  startPan(
    photoNode: PhotoRenderNode,
    event: FederatedPointerEvent,
  ) {
    const context = this.readContext();
    if (!this.acceptsGesture(photoNode.frameId, context)) return;
    const activeZoom = this.zoom;
    const continuesZoom = activeZoom?.gesture.frameId === photoNode.frameId;
    if (continuesZoom && activeZoom.timer !== null) {
      window.clearTimeout(activeZoom.timer);
      activeZoom.timer = null;
    }
    const currentZoom = continuesZoom
      ? activeZoom.gesture.baseZoom + activeZoom.gesture.delta
      : photoNode.baseZoom;
    this.pan = {
      generation: context.projectGeneration,
      frameId: photoNode.frameId,
      startX: event.global.x,
      startY: event.global.y,
      canvasScale: context.canvasScale,
      node: photoNode,
      originalX: photoNode.layer.x,
      originalY: photoNode.layer.y,
      targetX: photoNode.layer.x,
      targetY: photoNode.layer.y,
      currentX: photoNode.layer.x,
      currentY: photoNode.layer.y,
      // A settling wheel Zoom already moved the Photo around the Frame center.
      currentPan: continuesZoom
        ? photoNode.geometry.zoom(currentZoom).pan
        : photoNode.pan,
      currentZoom,
    };
    setPhotoPanAids(photoNode, true);
  }

  handleWheel(photoNode: PhotoRenderNode, event: FederatedWheelEvent) {
    const context = this.readContext();
    if (!context.input) return;
    event.preventDefault();
    const activePan = this.pan;
    if (activePan?.node.layer.destroyed) {
      this.dropGesture(activePan.frameId);
      return;
    }
    // During an Alt-Pan the wheel zooms the dragged Photo even when the
    // pointer drifts over another Frame, so its Zoom is never committed apart.
    const targetNode = activePan?.node ?? photoNode;
    if (!this.acceptsGesture(targetNode.frameId, context)) return;
    const current = this.zoom;
    if (current?.timer != null) {
      window.clearTimeout(current.timer);
    }
    const transition = advancePhotoZoomGesture(current?.gesture ?? null, {
      frameId: targetNode.frameId,
      baseZoom: targetNode.baseZoom,
      zoomRange: targetNode.geometry.zoomRange,
      wheelDeltaY: event.deltaY,
    });
    if (transition.interruptedCommit && current) {
      this.commitZoom(current.gesture, current.generation);
    }

    if (activePan) {
      const combined = targetNode.geometry.zoom(transition.previewZoom, {
        center: { x: activePan.currentX, y: activePan.currentY },
        zoom: activePan.currentZoom,
      });
      // The drag continues from where the Zoom moved the Photo, dropping any
      // distance it was dragged past the Pan limit.
      activePan.originalX += combined.placement.center.x - activePan.targetX;
      activePan.originalY += combined.placement.center.y - activePan.targetY;
      activePan.targetX = combined.placement.center.x;
      activePan.targetY = combined.placement.center.y;
      activePan.currentX = combined.placement.center.x;
      activePan.currentY = combined.placement.center.y;
      activePan.currentPan = combined.pan;
      activePan.currentZoom = combined.zoom;
      applyPhotoPlacementPreview(targetNode, combined.zoom, combined.placement);
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
        node: targetNode,
        timer: null,
      };
      return;
    }

    const zoomed = applyPhotoZoomPreview(photoNode, transition.previewZoom);
    context.input.onTransformPreview(
      createTransformPreview(photoNode.frameId, zoomed.pan, zoomed.zoom),
    );
    const runtime: ZoomGestureRuntime = {
      generation: context.projectGeneration,
      gesture: transition.gesture,
      node: photoNode,
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
      this.requestRender();
      this.zoom = null;
      if (!this.commitZoom(runtime.gesture, runtime.generation)) {
        currentContext.input?.onTransformPreview(null);
      }
    }, ZOOM_GESTURE_SETTLE_MS);
    this.zoom = runtime;
  }

  readonly handlePointerMove = (event: FederatedPointerEvent) => {
    const context = this.readContext();
    const gesture = this.pan;
    if (
      !gesture ||
      !context.input ||
      gesture.generation !== context.projectGeneration
    ) {
      return;
    }
    if (gesture.node.layer.destroyed) {
      this.dropGesture(gesture.frameId);
      return;
    }
    updatePanPreview(gesture, event.global.x, event.global.y);
    context.input.onTransformPreview(
      createTransformPreview(
        gesture.frameId,
        gesture.currentPan,
        gesture.currentZoom,
      ),
    );
  };

  readonly finishPan = (event: FederatedPointerEvent) => {
    const gesture = this.pan;
    if (!gesture) return;
    // Cleared first, so nothing below can leave a released drag in place.
    this.pan = null;
    const context = this.readContext();
    if (!context.input || gesture.generation !== context.projectGeneration) {
      return;
    }
    if (gesture.node.layer.destroyed) {
      this.dropGesture(gesture.frameId);
      return;
    }
    updatePanPreview(gesture, event.global.x, event.global.y);
    setPhotoPanAids(gesture.node, false);

    const deltaZoom = gesture.currentZoom - gesture.node.baseZoom;
    const combinedZoom = this.zoom;
    const ownsCombinedZoom = combinedZoom?.gesture.frameId === gesture.frameId;
    // The Core applies the Zoom around the Frame center before adding the Pan.
    const panBase =
      deltaZoom !== 0
        ? gesture.node.geometry.zoom(gesture.currentZoom).pan
        : gesture.node.pan;
    const deltaPanX = gesture.currentPan.x - panBase.x;
    const deltaPanY = gesture.currentPan.y - panBase.y;
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
        gesture.node.geometry.constrain(
          { x: gesture.currentX, y: gesture.currentY },
          gesture.currentZoom,
        ),
        gesture.generation,
      );
    } else {
      context.input.onTransformPreview(null);
    }
  };

  readonly cancelPan = () => {
    const gesture = this.pan;
    if (!gesture) return;
    if (!gesture.node.layer.destroyed) {
      setPhotoPanAids(gesture.node, false);
      resetPhotoPreview(gesture.node);
    }
    this.dropGesture(gesture.frameId);
  };

  /**
   * Runs after every scene update. A Sheet rebuild (another Frame's commit,
   * a preview texture loading) replaces the nodes under the previews.
   */
  applyPreviews() {
    const input = this.readContext().input;
    this.rebindGestures();
    this.rebindCommits(input?.revision);
    if (!input) return;
    const previousFrameId = this.externalPreviewFrameId;
    const nextPreview = input.photoZoomPreview;
    // A canvas gesture owns its Photo until it ends.
    const nextFrameId =
      nextPreview && !this.hasGesture(nextPreview.frameId)
        ? nextPreview.frameId
        : null;
    if (
      previousFrameId &&
      previousFrameId !== nextFrameId &&
      !this.hasGesture(previousFrameId)
    ) {
      const previousNode = this.photoNodes.get(previousFrameId);
      if (previousNode) resetPhotoPreview(previousNode);
    }
    const nextNode = nextFrameId ? this.photoNodes.get(nextFrameId) : null;
    if (nextPreview && nextNode) {
      applyPhotoZoomPreview(nextNode, nextPreview.value);
    }
    this.externalPreviewFrameId = nextFrameId;
  }

  /**
   * A rebuilt node of the same placement takes over the gesture at the shown
   * position; any other change, such as an Undo, ends the gesture.
   */
  private rebindGestures() {
    const pan = this.pan;
    const panNode = pan ? this.photoNodes.get(pan.frameId) : undefined;
    if (pan && panNode !== pan.node) {
      if (panNode && keepsPhotoPlacement(pan.node, panNode)) {
        pan.node = panNode;
        const shown = panNode.geometry.constrain(
          { x: pan.currentX, y: pan.currentY },
          pan.currentZoom,
        );
        applyPhotoPlacementPreview(panNode, shown.zoom, shown.placement);
        setPhotoPanAids(panNode, true);
      } else {
        this.dropGesture(pan.frameId);
      }
    }

    const zoom = this.zoom;
    const zoomNode = zoom ? this.photoNodes.get(zoom.gesture.frameId) : undefined;
    if (zoom && zoomNode !== zoom.node) {
      if (zoomNode && keepsPhotoPlacement(zoom.node, zoomNode)) {
        zoom.node = zoomNode;
        // A Zoom combined with the Pan was drawn with it above.
        if (this.pan?.frameId !== zoom.gesture.frameId) {
          applyPhotoZoomPreview(
            zoomNode,
            zoom.gesture.baseZoom + zoom.gesture.delta,
          );
        }
      } else {
        this.dropGesture(zoom.gesture.frameId);
      }
    }
  }

  /**
   * A commit keeps its preview on a rebuilt node of the same placement and
   * frees the Frame once the scene draws a new revision or placement.
   */
  private rebindCommits(revision: number | undefined) {
    for (const [frameId, commit] of this.commits) {
      const accepted = commit.acceptedAt !== undefined;
      const node = this.photoNodes.get(frameId);
      if (accepted && revision !== commit.acceptedAt) {
        this.commits.delete(frameId);
        // The drawn revision left this Sheet as it was, so the Photo shows
        // the placement the Project holds instead of the committed preview.
        if (node === commit.node) resetPhotoPreview(node);
      } else if (
        node &&
        node !== commit.node &&
        keepsPhotoPlacement(commit.node, node)
      ) {
        commit.node = node;
        applyPhotoPlacementPreview(
          node,
          commit.shown.zoom,
          commit.shown.placement,
        );
      } else if (accepted && node !== commit.node) {
        this.commits.delete(frameId);
      }
    }
  }

  private hasGesture(frameId: string) {
    return (
      this.pan?.frameId === frameId || this.zoom?.gesture.frameId === frameId
    );
  }

  /**
   * A Frame takes no new gesture while its commit is not drawn yet, nor while
   * the contextual panel holds a Zoom draft for it.
   */
  private acceptsGesture(frameId: string, context: PhotoInteractionContext) {
    return (
      !this.commits.has(frameId) &&
      context.input?.photoZoomPreview?.frameId !== frameId
    );
  }

  /** Forgets a gesture without touching its node, which may be destroyed. */
  private dropGesture(frameId: string) {
    if (this.pan?.frameId === frameId) this.pan = null;
    const zoom = this.zoom;
    if (zoom?.gesture.frameId === frameId) {
      if (zoom.timer !== null) window.clearTimeout(zoom.timer);
      this.zoom = null;
    }
    this.readContext().input?.onTransformPreview(null);
  }

  private commitZoom(gesture: PhotoZoomGesture, generation: number) {
    const commit = finishPhotoZoomGesture(gesture);
    const node = commit ? this.photoNodes.get(commit.frameId) : undefined;
    if (!commit || !node) return false;
    // The Core anchors the Zoom at the Frame center, as the preview did.
    this.commit(
      {
        frameId: commit.frameId,
        deltaPanX: 0,
        deltaPanY: 0,
        deltaZoom: commit.delta,
      },
      node,
      node.geometry.zoom(gesture.baseZoom + gesture.delta),
      generation,
    );
    return true;
  }

  private commit(
    delta: PhotoTransformDelta,
    node: PhotoRenderNode,
    shown: ConstrainedPhotoPlacement,
    generation: number,
  ) {
    const context = this.readContext();
    if (
      !context.input ||
      generation !== context.projectGeneration ||
      this.commits.has(delta.frameId)
    ) {
      return;
    }

    const entry: CommitPreview = { node, shown };
    this.commits.set(delta.frameId, entry);
    let result: Promise<boolean>;
    try {
      result = context.input.onTransformCommit(delta);
    } catch {
      this.settle(delta.frameId, entry, generation, false);
      return;
    }
    void result.then(
      (accepted) => this.settle(delta.frameId, entry, generation, accepted),
      () => this.settle(delta.frameId, entry, generation, false),
    );
  }

  private settle(
    frameId: string,
    entry: CommitPreview,
    generation: number,
    accepted: boolean,
  ) {
    const context = this.readContext();
    if (generation !== context.projectGeneration) return;
    this.requestRender();
    const tracked = this.commits.get(frameId) === entry;
    if (tracked) this.commits.delete(frameId);
    const shown = this.photoNodes.get(frameId) === entry.node;
    if (!accepted) {
      if (shown) {
        resetPhotoPreview(entry.node);
        context.input?.onTransformPreview(null);
      }
      return;
    }
    // React draws the committed projection in a later task. A host without
    // revisions cannot tell when, so its Frame is freed at once.
    const revision = context.input?.revision;
    if (tracked && shown && revision !== undefined) {
      entry.acceptedAt = revision;
      this.commits.set(frameId, entry);
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
  gesture.targetX =
    gesture.originalX + (pointerX - gesture.startX) / gesture.canvasScale;
  gesture.targetY =
    gesture.originalY + (pointerY - gesture.startY) / gesture.canvasScale;
  const constrained = gesture.node.geometry.constrain(
    { x: gesture.targetX, y: gesture.targetY },
    gesture.currentZoom,
  );
  gesture.currentX = constrained.placement.center.x;
  gesture.currentY = constrained.placement.center.y;
  gesture.currentPan = constrained.pan;
  setPhotoPreviewPosition(gesture.node, gesture.currentX, gesture.currentY);
}

/** The same committed Photo placement, drawn again by a rebuilt node. */
function keepsPhotoPlacement(
  previous: PhotoRenderNode,
  next: PhotoRenderNode,
) {
  const before = previous.geometry.current;
  const after = next.geometry.current;
  return [
    [previous.baseZoom, next.baseZoom],
    [previous.baseScaleX, next.baseScaleX],
    [previous.pan.x, next.pan.x],
    [previous.pan.y, next.pan.y],
    [before.center.x, after.center.x],
    [before.center.y, after.center.y],
    [before.size.width, after.size.width],
    [before.size.height, after.size.height],
  ].every(([a, b]) => Math.abs(a - b) <= 1e-9);
}
