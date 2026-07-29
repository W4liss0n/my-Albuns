import { UboSystem, loadTextures } from "pixi.js";
import { describe, expect, it } from "vitest";

import "./pixiCspRuntime";

describe("PixiJS CSP runtime", () => {
  it("installs the static uniform synchronizers required by the production CSP", () => {
    expect(UboSystem.prototype["_systemCheck"].toString()).not.toContain(
      "unsafeEvalSupported",
    );
  });

  it("avoids the Pixi worker capability probe that cannot complete under the CSP", () => {
    expect(loadTextures.config?.preferWorkers).toBe(false);
  });
});
