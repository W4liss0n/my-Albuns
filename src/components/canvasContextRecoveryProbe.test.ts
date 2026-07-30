import { describe, expect, test, vi } from "vitest";

import { runCanvasContextRecoveryProbe } from "./canvasContextRecoveryProbe";

describe("runCanvasContextRecoveryProbe", () => {
  test("measures limits and a rendered frame after controlled context recovery", async () => {
    const canvas = document.createElement("canvas");
    let now = 5;
    let contextLost = false;
    const extension = {
      loseContext: vi.fn(() => {
        contextLost = true;
        now = 15;
        canvas.dispatchEvent(
          new Event("webglcontextlost", { cancelable: true }),
        );
      }),
      restoreContext: vi.fn(() => {
        contextLost = false;
        now = 35;
        canvas.dispatchEvent(new Event("webglcontextrestored"));
      }),
    };
    const gl = {
      MAX_TEXTURE_SIZE: 0x0d33,
      MAX_RENDERBUFFER_SIZE: 0x84e8,
      MAX_TEXTURE_IMAGE_UNITS: 0x8872,
      getExtension: (name: string) =>
        name === "WEBGL_lose_context" ? extension : null,
      getParameter: (parameter: number) => {
        if (parameter === 0x0d33 || parameter === 0x84e8) {
          return 16_384;
        }
        if (parameter === 0x8872) return 16;
        return null;
      },
      isContextLost: () => contextLost,
      finish: vi.fn(),
      getError: () => 0,
    } as unknown as WebGL2RenderingContext;
    vi.spyOn(canvas, "getContext").mockImplementation(
      ((contextId: string) =>
        contextId === "webgl2" ? gl : null) as typeof canvas.getContext,
    );

    const measurement = await runCanvasContextRecoveryProbe(
      {
        canvas,
        renderAfterRestore: async () => {
          now = 47.5;
          return {
            renderedAt: now,
            textureBacked: true,
            decorativeTextureBacked: true,
            testedTexture: {
              mediaId: "decorative-overlay",
              widthPx: 1_600,
              heightPx: 1_200,
            },
          };
        },
      },
      { now: () => now },
    );

    expect(measurement).toEqual({
      webGlVersion: 2,
      maxTextureSizePx: 16_384,
      maxRenderbufferSizePx: 16_384,
      maxTextureImageUnits: 16,
      testedTexture: {
        mediaId: "decorative-overlay",
        widthPx: 1_600,
        heightPx: 1_200,
      },
      contextRecovery: {
        mechanism: "webgl_lose_context",
        contextLost: true,
        contextRestored: true,
        recoveryDurationMs: 32.5,
        restoredFrameLatencyMs: 12.5,
        glError: 0,
        textureBacked: true,
        decorativeTextureBacked: true,
      },
    });
    expect(extension.loseContext).toHaveBeenCalledOnce();
    expect(extension.restoreContext).toHaveBeenCalledOnce();
    expect(gl.finish).toHaveBeenCalledOnce();
  });

  test("fails clearly when controlled context loss is unavailable", async () => {
    const canvas = document.createElement("canvas");
    const gl = {
      getExtension: () => null,
    } as unknown as WebGL2RenderingContext;
    vi.spyOn(canvas, "getContext").mockImplementation(
      ((contextId: string) =>
        contextId === "webgl2" ? gl : null) as typeof canvas.getContext,
    );

    await expect(
      runCanvasContextRecoveryProbe(
        {
          canvas,
          renderAfterRestore: vi.fn(),
        },
        { now: () => 0 },
      ),
    ).rejects.toThrow(
      "WEBGL_lose_context não está disponível no Canvas real.",
    );
  });

  test("waits for the loss task to finish before requesting restoration", async () => {
    const canvas = document.createElement("canvas");
    let contextLost = false;
    let restorationAllowed = false;
    const extension = {
      loseContext: vi.fn(() => {
        contextLost = true;
        canvas.dispatchEvent(
          new Event("webglcontextlost", { cancelable: true }),
        );
        globalThis.setTimeout(() => {
          restorationAllowed = true;
        }, 0);
      }),
      restoreContext: vi.fn(() => {
        if (!restorationAllowed) return;
        contextLost = false;
        canvas.dispatchEvent(new Event("webglcontextrestored"));
      }),
    };
    const gl = {
      MAX_TEXTURE_SIZE: 0x0d33,
      MAX_RENDERBUFFER_SIZE: 0x84e8,
      MAX_TEXTURE_IMAGE_UNITS: 0x8872,
      getExtension: () => extension,
      getParameter: () => 16_384,
      isContextLost: () => contextLost,
      finish: vi.fn(),
      getError: () => 0,
    } as unknown as WebGL2RenderingContext;
    vi.spyOn(canvas, "getContext").mockImplementation(
      ((contextId: string) =>
        contextId === "webgl2" ? gl : null) as typeof canvas.getContext,
    );
    const controller = new AbortController();
    const abortTimer = globalThis.setTimeout(
      () => controller.abort(),
      50,
    );

    try {
      await expect(
        runCanvasContextRecoveryProbe(
          {
            canvas,
            renderAfterRestore: async () => ({
              renderedAt: 10,
              textureBacked: true,
              decorativeTextureBacked: true,
              testedTexture: {
                mediaId: "decorative-overlay",
                widthPx: 1_600,
                heightPx: 1_200,
              },
            }),
          },
          { now: () => 0 },
          controller.signal,
        ),
      ).resolves.toMatchObject({
        contextRecovery: {
          contextLost: true,
          contextRestored: true,
        },
      });
    } finally {
      globalThis.clearTimeout(abortTimer);
    }
  });

  test("honors cancellation while waiting for a context event", async () => {
    const canvas = document.createElement("canvas");
    const extension = {
      loseContext: vi.fn(),
      restoreContext: vi.fn(),
    };
    const gl = {
      MAX_TEXTURE_SIZE: 0x0d33,
      MAX_RENDERBUFFER_SIZE: 0x84e8,
      MAX_TEXTURE_IMAGE_UNITS: 0x8872,
      getExtension: () => extension,
      getParameter: () => 16_384,
      isContextLost: () => false,
    } as unknown as WebGL2RenderingContext;
    vi.spyOn(canvas, "getContext").mockImplementation(
      ((contextId: string) =>
        contextId === "webgl2" ? gl : null) as typeof canvas.getContext,
    );
    const controller = new AbortController();

    const probe = runCanvasContextRecoveryProbe(
      {
        canvas,
        renderAfterRestore: vi.fn(),
      },
      { now: () => 0 },
      controller.signal,
    );
    controller.abort();

    await expect(probe).rejects.toMatchObject({
      name: "AbortError",
    });
    expect(extension.restoreContext).not.toHaveBeenCalled();
  });

  test("attempts emergency restoration without masking an abort after context loss", async () => {
    const canvas = document.createElement("canvas");
    const controller = new AbortController();
    let contextLost = false;
    let restorationAllowed = false;
    const extension = {
      loseContext: vi.fn(() => {
        contextLost = true;
        canvas.dispatchEvent(
          new Event("webglcontextlost", { cancelable: true }),
        );
        globalThis.setTimeout(() => {
          restorationAllowed = true;
        }, 0);
        controller.abort();
      }),
      restoreContext: vi.fn(() => {
        if (!restorationAllowed) return;
        contextLost = false;
        throw new Error("falha secundária ao restaurar");
      }),
    };
    const gl = {
      MAX_TEXTURE_SIZE: 0x0d33,
      MAX_RENDERBUFFER_SIZE: 0x84e8,
      MAX_TEXTURE_IMAGE_UNITS: 0x8872,
      getExtension: () => extension,
      getParameter: () => 16_384,
      isContextLost: () => contextLost,
    } as unknown as WebGL2RenderingContext;
    vi.spyOn(canvas, "getContext").mockImplementation(
      ((contextId: string) =>
        contextId === "webgl2" ? gl : null) as typeof canvas.getContext,
    );

    await expect(
      runCanvasContextRecoveryProbe(
        {
          canvas,
          renderAfterRestore: vi.fn(),
        },
        { now: () => 0 },
        controller.signal,
      ),
    ).rejects.toMatchObject({
      name: "AbortError",
    });
    expect(extension.restoreContext).toHaveBeenCalledOnce();
    expect(contextLost).toBe(false);
  });
});
