import { act, render } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { CompositionPlan } from "../domain/project";
import { AlbumCanvas } from "./AlbumCanvas";

const pixiLifecycle = vi.hoisted(() => ({
  instances: [] as Array<{
    initialized: boolean;
    destroyCount: number;
  }>,
  resolveInitializations: [] as Array<() => void>,
}));

vi.mock("pixi.js", () => {
  class Application {
    canvas = document.createElement("canvas");
    initialized = false;
    destroyCount = 0;

    constructor() {
      pixiLifecycle.instances.push(this);
    }

    init() {
      return new Promise<void>((resolve) => {
        pixiLifecycle.resolveInitializations.push(() => {
          this.initialized = true;
          resolve();
        });
      });
    }

    destroy() {
      if (!this.initialized) {
        throw new TypeError("PixiJS was destroyed before initialization");
      }
      this.destroyCount += 1;
    }
  }

  return {
    Application,
    Container: class {},
    FederatedPointerEvent: class {},
    FederatedWheelEvent: class {},
    Graphics: class {},
    Rectangle: class {},
    Text: class {},
  };
});

const composition: CompositionPlan = {
  sheets: [
    {
      sheetId: "sheet-001",
      number: 1,
      widthUm: 600_000,
      heightUm: 300_000,
      hasOverlay: false,
      frames: [],
    },
  ],
};

test("waits for PixiJS initialization before destroying an abandoned Canvas", async () => {
  const view = render(
    <AlbumCanvas
      composition={composition}
      selectedFrameId={null}
      focusedSheetId="sheet-001"
      viewport={{ offsetX: 0, zoom: 1 }}
      onSelectFrame={() => undefined}
      onFocusSheet={() => undefined}
      onViewportChange={() => undefined}
      onPanCommit={() => undefined}
      onZoomCommit={() => undefined}
      onMaterializedChange={() => undefined}
    />,
  );

  expect(pixiLifecycle.instances).toHaveLength(1);
  expect(() => view.unmount()).not.toThrow();

  await act(async () => {
    pixiLifecycle.resolveInitializations[0]();
    await Promise.resolve();
  });

  expect(pixiLifecycle.instances[0].destroyCount).toBe(1);
});
