// Browser regression fixture: production Pixi scene with a delayed deterministic
// port. It verifies presentation timing and cursors, not Core geometry policy.
import { Application, Container, Rectangle } from "pixi.js";
import type { ComposedFrame, FrameGeometryEdit } from "../domain/project";
import type { AlbumCanvasProps } from "../components/albumCanvasContract";
import { AlbumCanvasScene } from "../components/albumCanvasScene";
import { createContinuousCanvasLayout } from "../components/canvasGeometry";
import { interactiveComposition } from "../components/albumCanvasTestFixtures";
import "../components/AlbumCanvas.css";
import "../components/pixiRuntime";
import { useEditorView } from "../state/editorView";
import { frameGeometryPreview } from "./frameGeometryPreview";

const app = new Application();
await app.init({ width: 900, height: 600, background: "#ddd", preference: "webgl" });
app.canvas.className = "pixi-canvas";
document.body.appendChild(app.canvas);
const scene = new AlbumCanvasScene(app);
const composition = structuredClone(interactiveComposition);
composition.sheets[0].frames[0].clipRect = {
  x: 120_000, y: 50_000, width: 240_000, height: 160_000,
};
const group = window.location.search.includes("group");
if (group) composition.sheets[0].frames.push({ ...composition.sheets[0].frames[0],
  frameId: "frame-002", zIndex: 1, photo: null,
  clipRect: { x: 200_000, y: 100_000, width: 180_000, height: 100_000 },
});
const originals = structuredClone(composition.sheets[0].frames);
useEditorView.setState({ editingSheetId: "sheet-001", selectedFrameIds: group ? [] : ["frame-001"] });
const noop = () => undefined;
const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
const trace: ReturnType<typeof sample>[] = [];
const completion = { committedAt: null as number | null, presentedAt: null as number | null };
const input: AlbumCanvasProps = {
  projectId: "frame-gesture-regression",
  mode: { kind: "sheet-editing", sheetId: "sheet-001" },
  composition,
  sheetBarMetadata: [{ sheetId: "sheet-001", pageNumbers: [1, 2], layoutLocked: false }],
  continuousCanvasLayout: createContinuousCanvasLayout(composition.sheets),
  selectedFrameIds: useEditorView.getState().selectedFrameIds, focusedSheetId: "sheet-001", centeredSheetId: "sheet-001",
  viewport: { offsetX: 0 },
  onSelectFrame: useEditorView.getState().selectFrame, onEditSheet: noop, onFocusSheet: noop,
  onCenteredSheetChange: noop, onViewportChange: noop, onTransformPreview: noop,
  onTransformCommit: async () => true,
  frameGeometry: {
    disabled: false,
    dragThreshold: { x: 5, y: 5 },
    preview: async (edit) => { await wait(60); return frameGeometryPreview(proposed(edit)); },
    commit: async (edit) => {
      await wait(30);
      const committed = proposed(edit);
      setTimeout(() => {
        input.composition = structuredClone(composition);
        input.composition.sheets[0].frames = committed;
        scene.update(input, 600);
        completion.presentedAt = performance.now();
      }, 70);
      completion.committedAt = performance.now();
      return committed;
    },
    onError: (message) => { throw new Error(message); },
  },
};

function proposed(edit: FrameGeometryEdit): ComposedFrame[] {
  const frames = originals.filter((frame) => edit.frames.some((target) => target.frameId === frame.frameId));
  const left = Math.min(...frames.map((frame) => frame.clipRect.x));
  const width = Math.max(...frames.map((frame) => frame.clipRect.x + frame.clipRect.width)) - left;
  return frames.map((original) => {
    const frame = structuredClone(original);
    if (edit.gesture.kind === "move") {
      frame.clipRect.x += edit.gesture.deltaXUm;
      frame.clipRect.y += edit.gesture.deltaYUm;
    } else if (edit.gesture.handle === "right") {
      const scale = (width + edit.gesture.deltaXUm) / width;
      frame.clipRect.x = left + Math.round((frame.clipRect.x - left) * scale);
      frame.clipRect.width = Math.round(frame.clipRect.width * scale);
    } else throw new Error("This rendering fixture only exercises move and the right resize handle.");
    return frame;
  });
}
useEditorView.subscribe((state) => {
  input.selectedFrameIds = state.selectedFrameIds;
  scene.update(input, 600);
});

function find(label: string, parent: Container = app.stage): Container | undefined {
  if (parent.label === label) return parent;
  for (const child of parent.children) {
    const found = find(label, child);
    if (found) return found;
  }
}

function sample() {
  const frame = find("canvas-frame-frame-001");
  const rect = frame?.getBounds();
  const hitArea = frame?.hitArea instanceof Rectangle ? frame.hitArea : undefined;
  return {
    cursor: getComputedStyle(app.canvas).cursor,
    x: rect?.x, y: rect?.y, width: hitArea?.width,
    members: originals.map((original) => {
      const node = find(`canvas-frame-${original.frameId}`);
      return { x: node?.position.x, y: node?.position.y, width: node?.hitArea instanceof Rectangle ? node.hitArea.width : null };
    }),
    time: performance.now(),
  };
}

scene.update(input, 600);
let recording = false;
app.ticker.add(() => { if (recording) trace.push(sample()); });
Object.assign(window, {
  frameGestureTest: {
    start: () => { recording = true; trace.length = 0; },
    sample,
    selection: () => input.selectedFrameIds,
    trace: () => trace,
    completion: () => completion,
    point: (action: string) => {
      const frame = find("canvas-frame-frame-001");
      const target = action === "resize" ? find(group ? "frame-resize-handle-right-group-sheet-001" : "frame-resize-handle-right-frame-001")
        : action === "overlap" ? find("canvas-frame-frame-002") : frame;
      if (!target) throw new Error("Frame target was not rendered.");
      const point = target.toGlobal(action === "resize" ? { x: 0, y: 0 } : action === "empty" ? { x: -70, y: -20 } : { x: 30, y: 12 });
      const bounds = app.canvas.getBoundingClientRect();
      return {
        x: bounds.left + point.x * bounds.width / app.screen.width,
        y: bounds.top + point.y * bounds.height / app.screen.height,
      };
    },
  },
});
document.body.dataset.ready = "true";
