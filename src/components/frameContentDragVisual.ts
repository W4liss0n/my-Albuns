import { Container, Graphics, Sprite, type Application, type Texture } from "pixi.js";
import type { PhotoRenderNode } from "./albumCanvasRenderNodes";
import type { FrameContentDragPreview } from "./frameContentDragSession";
import { IMAGE_DRAG_GHOST_STYLE, imageDragGhostGeometry, imageDragGhostPosition } from "../ui/imageDragGhostVisual";

/** Owns one bounded snapshot, so the ghost survives source-Sheet virtualization. */
export class FrameContentDragVisual {
  private readonly container = new Container();
  private texture: Texture | null = null;
  private sourceFrameId: string | null = null;
  private width = 0;
  private height = 0;

  constructor(private readonly app: Application, private readonly photoNodes: ReadonlyMap<string, PhotoRenderNode>) {
    this.container.label = "frame-content-drag-ghost";
    this.container.eventMode = "none";
    this.container.alpha = IMAGE_DRAG_GHOST_STYLE.opacity;
    this.container.visible = false;
  }

  update(preview: FrameContentDragPreview | null) {
    if (!preview) { this.reset(); return; }
    if (preview.sourceFrameId !== this.sourceFrameId) {
      this.reset();
      const source = this.photoNodes.get(preview.sourceFrameId);
      if (!source) return;
      const style = IMAGE_DRAG_GHOST_STYLE;
      const snapshot = source.createDragPreview();
      const { width, height, scale, border, shadow } = imageDragGhostGeometry(snapshot.bounds.width, snapshot.bounds.height);
      this.width = width;
      this.height = height;
      try {
        this.texture = this.app.renderer.generateTexture({ target: snapshot.container,
          frame: snapshot.bounds, resolution: scale * this.app.renderer.resolution });
      } finally { snapshot.container.destroy({ children: true }); }
      const photo = new Sprite({ texture: this.texture });
      photo.width = this.width;
      photo.height = this.height;
      this.container.addChild(
        new Graphics().rect(shadow.x, shadow.y, shadow.width, shadow.height)
          .fill({ color: style.shadow.color, alpha: style.shadow.opacity }),
        new Graphics().rect(border.x, border.y, border.width, border.height).fill(style.border.color),
        photo,
      );
      this.sourceFrameId = preview.sourceFrameId;
      this.app.stage.addChild(this.container);
    }
    const bounds = this.app.canvas.getBoundingClientRect();
    const position = imageDragGhostPosition({
      x: (preview.clientX - bounds.left) * this.app.screen.width / bounds.width,
      y: (preview.clientY - bounds.top) * this.app.screen.height / bounds.height,
    }, { width: this.width, height: this.height }, this.app.screen);
    this.container.position.set(position.x, position.y);
    this.container.visible = true;
    this.app.canvas.dataset.frameContentDragGhost = preview.sourceFrameId;
  }

  reset() {
    this.container.visible = false;
    delete this.app.canvas.dataset.frameContentDragGhost;
    if (this.sourceFrameId !== null) this.app.stage.removeChild(this.container);
    for (const child of this.container.removeChildren()) child.destroy();
    this.texture?.destroy(true);
    this.texture = null;
    this.sourceFrameId = null;
  }

  destroy() {
    this.reset();
    this.container.destroy();
  }
}
