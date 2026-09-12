import { act } from "@testing-library/react";
import { expect, test } from "vitest";
import type { CompositionPlan } from "../domain/project";
import { interactiveComposition } from "./albumCanvasTestFixtures";
import { finishPixiInitialization, getPixiLifecycle, renderCanvas, setupAlbumCanvasTestHarness } from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();

function renderedTextures() {
  type Node = { texture?: unknown; children?: Node[] };
  const visit = (node: Node): unknown[] => [
    ...(node.texture ? [node.texture] : []),
    ...(node.children ?? []).flatMap(visit),
  ];
  return visit(getPixiLifecycle().instances[0].stage as Node);
}

test.each(["photo", "background", "overlay"] as const)(
  "retains %s pixels during replacement and ignores a superseded texture load",
  async (role) => {
    const lifecycle = getPixiLifecycle();
    const plan: CompositionPlan = structuredClone(interactiveComposition);
    const sheet = plan.sheets[0];
    const mediaId = role === "photo" ? sheet.frames[0].photo!.mediaId : "decorative-001";
    if (role !== "photo") {
      sheet.frames = [];
      const content = { mediaId, name: "Textura", drawRect: { x: 0, y: 0, width: sheet.widthUm, height: sheet.heightUm } };
      if (role === "background") sheet.backgrounds = [{ kind: "media", ...content }];
      else sheet.overlays = [content];
    }
    const previousUrl = "asset://cache/previous.jpg";
    const supersededUrl = "asset://cache/superseded.jpg";
    const latestUrl = "asset://cache/latest.jpg";
    const previous = { label: "previous" };
    const superseded = { label: "superseded" };
    const latest = { label: "latest" };
    const view = renderCanvas({ compositionPlan: plan, mediaPreviewUrls: { [mediaId]: previousUrl } });
    await finishPixiInitialization();
    await act(async () => lifecycle.resolveAssetLoads[0](previous));
    expect(renderedTextures()).toContain(previous);

    view.rerenderCanvas({ mediaPreviewUrls: { [mediaId]: supersededUrl } });
    expect(renderedTextures()).toContain(previous);
    expect(lifecycle.assetUnloads).not.toContain(previousUrl);
    view.rerenderCanvas({ mediaPreviewUrls: { [mediaId]: latestUrl } });
    await act(async () => lifecycle.resolveAssetLoads[1](superseded));
    expect(renderedTextures()).toContain(previous);
    expect(renderedTextures()).not.toContain(superseded);
    expect(lifecycle.assetUnloads).toContain(supersededUrl);

    await act(async () => lifecycle.resolveAssetLoads[2](latest));
    expect(renderedTextures()).toContain(latest);
    expect(renderedTextures()).not.toContain(previous);
    expect(lifecycle.assetUnloads).toContain(previousUrl);

    view.rerenderCanvas({ mediaPreviewUrls: {} });
    expect(renderedTextures()).not.toContain(latest);
    expect(lifecycle.assetUnloads).toContain(latestUrl);
  },
);

test("releases the retained generation when the Project changes during replacement", async () => {
  const lifecycle = getPixiLifecycle();
  const mediaId = interactiveComposition.sheets[0].frames[0].photo!.mediaId;
  const previous = { label: "previous-project" };
  const view = renderCanvas({ compositionPlan: interactiveComposition,
    mediaPreviewUrls: { [mediaId]: "asset://cache/previous.jpg" } });
  await finishPixiInitialization();
  await act(async () => lifecycle.resolveAssetLoads[0](previous));
  view.rerenderCanvas({ mediaPreviewUrls: { [mediaId]: "asset://cache/pending.jpg" } });
  expect(renderedTextures()).toContain(previous);
  view.rerenderCanvas({ projectId: "another-project" });
  expect(renderedTextures()).not.toContain(previous);
  expect(lifecycle.assetUnloads).toContain("asset://cache/previous.jpg");
});
