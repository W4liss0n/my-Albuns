import { afterEach, expect, test, vi } from "vitest";

import { probeGraphics } from "./graphics";

afterEach(() => {
  vi.restoreAllMocks();
});

test("confirms hardware WebGL2 with its texture limits and releases the probe context", () => {
  const loseContext = vi.fn();
  const context = fakeContext({
    renderer: "ANGLE (NVIDIA GeForce RTX 3050)",
    loseContext,
  });
  vi.spyOn(
    HTMLCanvasElement.prototype,
    "getContext",
  ).mockImplementation(((contextId: string) =>
    contextId === "webgl2" ? context : null) as typeof HTMLCanvasElement.prototype.getContext);

  expect(probeGraphics()).toEqual({
    supported: true,
    renderer: "ANGLE (NVIDIA GeForce RTX 3050)",
    reason: "WebGL2 acelerado por hardware confirmado.",
    limits: {
      maxTextureSizePx: 16_384,
      maxRenderbufferSizePx: 16_384,
      maxTextureImageUnits: 16,
    },
  });
  expect(loseContext).toHaveBeenCalledOnce();
});

test("distinguishes absent, inconclusive, and software WebGL2", () => {
  const cases = [
    {
      context: null,
      code: "webgl2_unavailable",
      renderer: "indisponível",
      limits: null,
    },
    {
      context: fakeContext({
        renderer: "ignored",
        loseContext: vi.fn(),
        debugRenderer: false,
      }),
      code: "hardware_unconfirmed",
      renderer: "não confirmado",
      limits: {
        maxTextureSizePx: 16_384,
        maxRenderbufferSizePx: 16_384,
        maxTextureImageUnits: 16,
      },
    },
    {
      context: fakeContext({
        renderer: "Google SwiftShader",
        loseContext: vi.fn(),
      }),
      code: "software_renderer",
      renderer: "Google SwiftShader",
      limits: {
        maxTextureSizePx: 16_384,
        maxRenderbufferSizePx: 16_384,
        maxTextureImageUnits: 16,
      },
    },
    {
      context: fakeContext({
        renderer: "ANGLE (NVIDIA GeForce RTX 3050)",
        loseContext: vi.fn(),
        maxTextureSizePx: 0,
      }),
      code: "hardware_unconfirmed",
      renderer: "não confirmado",
      limits: null,
    },
    {
      context: fakeContext({
        renderer: "",
        loseContext: vi.fn(),
      }),
      code: "hardware_unconfirmed",
      renderer: "não confirmado",
      limits: {
        maxTextureSizePx: 16_384,
        maxRenderbufferSizePx: 16_384,
        maxTextureImageUnits: 16,
      },
    },
  ] as const;

  for (const candidate of cases) {
    vi.spyOn(
      HTMLCanvasElement.prototype,
      "getContext",
    ).mockImplementation(((contextId: string) =>
      contextId === "webgl2"
        ? candidate.context
        : null) as typeof HTMLCanvasElement.prototype.getContext);

    expect(probeGraphics()).toEqual(
      expect.objectContaining({
        supported: false,
        code: candidate.code,
        renderer: candidate.renderer,
        limits: candidate.limits,
      }),
    );
    vi.restoreAllMocks();
  }
});

function fakeContext({
  renderer,
  loseContext,
  debugRenderer = true,
  maxTextureSizePx = 16_384,
}: {
  renderer: string;
  loseContext: () => void;
  debugRenderer?: boolean;
  maxTextureSizePx?: number;
}) {
  const constants = {
    MAX_TEXTURE_SIZE: 0x0d33,
    MAX_RENDERBUFFER_SIZE: 0x84e8,
    MAX_TEXTURE_IMAGE_UNITS: 0x8872,
  };
  const rendererParameter = 0x9246;
  return {
    ...constants,
    getExtension(name: string) {
      if (name === "WEBGL_debug_renderer_info" && debugRenderer) {
        return { UNMASKED_RENDERER_WEBGL: rendererParameter };
      }
      if (name === "WEBGL_lose_context") {
        return { loseContext };
      }
      return null;
    },
    getParameter(parameter: number) {
      if (parameter === rendererParameter) return renderer;
      if (
        parameter === constants.MAX_TEXTURE_SIZE
      ) {
        return maxTextureSizePx;
      }
      if (parameter === constants.MAX_RENDERBUFFER_SIZE) return 16_384;
      if (parameter === constants.MAX_TEXTURE_IMAGE_UNITS) return 16;
      return null;
    },
  } as unknown as WebGL2RenderingContext;
}
