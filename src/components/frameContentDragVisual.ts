import { Container, Graphics, Sprite, type Application, type Texture } from "pixi.js";
import type { PhotoRenderNode } from "./albumCanvasRenderNodes";
import type { FrameContentDragPreview } from "./frameContentDragSession";
import { SHEET_VISUAL_STYLE } from "./sheetVisualStyle";

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
    this.container.alpha = SHEET_VISUAL_STYLE.frameContentDrag.ghostOpacity;
    this.container.visible = false;
  }

  update(preview: FrameContentDragPreview | null) {
    if (!preview) { this.reset(); return; }
    if (preview.sourceFrameId !== this.sourceFrameId) {
      this.reset();
      const source = this.photoNodes.get(preview.sourceFrameId);
      if (!source) return;
      const style = SHEET_VISUAL_STYLE.frameContentDrag;
      const scale = Math.min(1, style.ghostMaxWidthPx / source.clipRect.width,
        style.ghostMaxHeightPx / source.clipRect.height);
      this.width = source.clipRect.width * scale;
      this.height = source.clipRect.height * scale;
      // Keep masks and render transforms in a detached tree. Rendering a subtree
      // whose mask belongs to the live Sheet can corrupt its cached transforms.
      const snapshot = source.createDragPreview();
      try {
        this.texture = this.app.renderer.generateTexture({ target: snapshot,
          frame: source.clipRect, resolution: scale * this.app.renderer.resolution });
      } finally { snapshot.destroy({ children: true }); }
      const photo = new Sprite({ texture: this.texture });
      photo.width = this.width;
      photo.height = this.height;
      this.container.addChild(
        new Graphics().rect(3, 5, this.width + 2, this.height + 2).fill({ color: 0x252525, alpha: 0.2 }),
        new Graphics().rect(-2, -2, this.width + 4, this.height + 4).fill(0xffffff),
        photo,
      );
      this.sourceFrameId = preview.sourceFrameId;
      this.app.stage.addChild(this.container);
    }
    const bounds = this.app.canvas.getBoundingClientRect();
    const offset = SHEET_VISUAL_STYLE.frameContentDrag.ghostPointerOffsetPx;
    this.container.position.set(
      Math.max(8, Math.min((preview.clientX - bounds.left) * this.app.screen.width / bounds.width + offset,
        this.app.screen.width - this.width - 8)),
      Math.max(8, Math.min((preview.clientY - bounds.top) * this.app.screen.height / bounds.height + offset,
        this.app.screen.height - this.height - 8)),
    );
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
