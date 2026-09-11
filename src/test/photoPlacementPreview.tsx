// Exercises first-render pixels through the production thumbnail and Pixi scene.
import { Application, Container } from "pixi.js";
import { createRoot } from "react-dom/client";
import { MediaThumbnail } from "../components/MediaThumbnail";
import { AlbumCanvasScene } from "../components/albumCanvasScene";
import { interactiveComposition } from "../components/albumCanvasTestFixtures";
import type { AlbumCanvasProps } from "../components/albumCanvasContract";
import { createContinuousCanvasLayout } from "../components/canvasGeometry";
import { loadedMediaPreviewImage } from "../application/mediaPreviewImages";
import "../components/pixiRuntime";

const count = 12;
const previews = Array.from({ length: count }, (_, index) => {
  const canvas = document.createElement("canvas");
  canvas.width = 600;
  canvas.height = 400;
  const context = canvas.getContext("2d")!;
  const rgb = [20 + index, 183, 103];
  context.fillStyle = `rgb(${rgb.join(",")})`;
  context.fillRect(0, 0, canvas.width, canvas.height);
  return { url: canvas.toDataURL(index % 2 === 0 ? "image/png" : "image/jpeg", 1), rgb };
});
const svg = { url: `data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="600" height="400"><rect width="600" height="400" fill="#7432a0"/></svg>')}`, rgb: [116, 50, 160] };
const thumbnails = createRoot(document.getElementById("thumbnails")!);
thumbnails.render([...previews, svg].map(({ url }, index) => <MediaThumbnail key={index}
  previewUrl={url} loading="eager" media={{ sourceWidthPx: 600, sourceHeightPx: 400 }} />));
const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
for (let attempt = 0; attempt < 200 && previews.some(({ url }) => !loadedMediaPreviewImage(url)); attempt += 1) await wait(10);
if (previews.some(({ url }) => !loadedMediaPreviewImage(url))) throw new Error("Thumbnails did not load.");
for (const preview of previews) {
  // Compare against decoded thumbnail pixels, including JPEG quantization.
  const reference = document.createElement("canvas");
  reference.width = reference.height = 1;
  const context = reference.getContext("2d")!;
  context.drawImage(loadedMediaPreviewImage(preview.url)!, 0, 0, 1, 1);
  preview.rgb = Array.from(context.getImageData(0, 0, 1, 1).data).slice(0, 3);
}

const app = new Application();
await app.init({ width: 900, height: 500, background: "#ddd", preference: "webgl" });
document.body.appendChild(app.canvas);
app.stop();
const scene = new AlbumCanvasScene(app);
const base = interactiveComposition.sheets[0];
const noop = () => undefined;
const input: AlbumCanvasProps = {
  projectId: "photo-placement-regression", mode: { kind: "normal" },
  composition: { ...interactiveComposition, sheets: [{ ...base, frames: [] }] },
  continuousCanvasLayout: createContinuousCanvasLayout([base]), sheetBarMetadata: [],
  selectedFrameIds: [], focusedSheetId: base.sheetId, centeredSheetId: base.sheetId,
  viewport: { offsetX: 0 }, mediaPreviewUrls: Object.fromEntries(previews.map(({ url }, index) => [`photo-${index}`, url])),
  onSelectFrame: noop, onEditSheet: noop, onFocusSheet: noop, onCenteredSheetChange: noop,
  onViewportChange: noop, onTransformPreview: noop, onTransformCommit: async () => true,
};
scene.update(input, 500);
function find(label: string, node: Container = app.stage): Container | undefined {
  if (node.label === label) return node;
  for (const child of node.children) { const found = find(label, child); if (found) return found; }
}
const samples: { index: number; first: number[]; settled: number[]; expected: number[] }[] = [];
function sample(index: number) {
  const frame = find(`canvas-frame-photo-${index}`)!;
  const canvas = app.renderer.extract.canvas({ target: frame, resolution: 1 });
  return Array.from(canvas.getContext("2d")!.getImageData(Math.floor(canvas.width / 2), Math.floor(canvas.height / 2), 1, 1).data);
}
// Opening starts before Cache URLs arrive, then waits for the texture decode.
// The Photo area must stay free of demonstration artwork in both intervals.
input.composition.sheets[0].frames.push({ ...base.frames[0], frameId: "photo-0",
  photo: { ...base.frames[0].photo!, mediaId: "opening-photo" } });
scene.update(input, 500);
const openingSample = { beforeUrl: sample(0), beforeTexture: [] as number[], ready: [] as number[], expected: [...svg.rgb, 255] };
input.mediaPreviewUrls = { ...input.mediaPreviewUrls, "opening-photo": svg.url };
scene.update(input, 500);
openingSample.beforeTexture = sample(0);
for (let attempt = 0; attempt < 200 && sample(0)[3] !== 255; attempt += 1) await wait(10);
openingSample.ready = sample(0);
input.composition.sheets[0].frames = [];
scene.update(input, 500);

for (let index = 0; index < count; index += 1) {
  input.composition.sheets[0].frames.push({ ...base.frames[0], frameId: `photo-${index}`, zIndex: index,
    clipRect: { x: (index % 4) * 150_000, y: Math.floor(index / 4) * 100_000, width: 150_000, height: 100_000 },
    photo: { ...base.frames[0].photo!, mediaId: `photo-${index}` },
  });
  input.selectedFrameIds = [`photo-${index}`];
  scene.update(input, 500);
  const first = sample(index);
  await wait(40);
  samples.push({ index, first, settled: sample(index), expected: [...previews[index].rgb, 255] });
}
// Development fixtures use SVG: they still need Pixi's rasterizing loader.
input.mediaPreviewUrls = { ...input.mediaPreviewUrls, svg: svg.url };
input.composition.sheets[0].frames[0].photo!.mediaId = "svg";
scene.update(input, 500);
await wait(300);
const svgSample = { actual: sample(0), expected: [...svg.rgb, 255] };
app.render();
Object.assign(window, { photoPlacementTest: { samples, svgSample, openingSample } });
document.body.dataset.ready = "true";
