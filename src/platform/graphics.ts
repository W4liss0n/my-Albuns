import type { GraphicsDiagnostic } from "../application/graphics";

const SOFTWARE_RENDERERS = [
  "swiftshader",
  "llvmpipe",
  "software",
  "microsoft basic render",
  "warp",
];

export function probeGraphics(): GraphicsDiagnostic {
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("webgl2", {
    failIfMajorPerformanceCaveat: true,
    powerPreference: "high-performance",
  });
  try {
    return diagnoseWebGl2Context(context);
  } finally {
    context
      ?.getExtension("WEBGL_lose_context")
      ?.loseContext();
  }
}

export function probeCanvasGraphics(
  canvas: HTMLCanvasElement,
): GraphicsDiagnostic {
  return diagnoseWebGl2Context(canvas.getContext("webgl2"));
}

function diagnoseWebGl2Context(
  context: WebGL2RenderingContext | null,
): GraphicsDiagnostic {
  if (!context) {
    return {
      supported: false,
      code: "webgl2_unavailable",
      renderer: "indisponível",
      reason:
        "Não foi possível criar um contexto WebGL2 acelerado por hardware.",
      limits: null,
    };
  }

  const limits = {
    maxTextureSizePx: Number(
      context.getParameter(context.MAX_TEXTURE_SIZE),
    ),
    maxRenderbufferSizePx: Number(
      context.getParameter(context.MAX_RENDERBUFFER_SIZE),
    ),
    maxTextureImageUnits: Number(
      context.getParameter(context.MAX_TEXTURE_IMAGE_UNITS),
    ),
  };
  if (
    !Object.values(limits).every(
      (value) => Number.isInteger(value) && value > 0,
    )
  ) {
    return {
      supported: false,
      code: "hardware_unconfirmed",
      renderer: "não confirmado",
      reason:
        "WebGL2 existe, mas seus limites gráficos não puderam ser confirmados.",
      limits: null,
    };
  }
  const rendererExtension = context.getExtension("WEBGL_debug_renderer_info");
  if (!rendererExtension) {
    return {
      supported: false,
      code: "hardware_unconfirmed",
      renderer: "não confirmado",
      reason:
        "WebGL2 existe, mas o backend de hardware não pôde ser confirmado.",
      limits,
    };
  }

  const rendererValue = context.getParameter(
    rendererExtension.UNMASKED_RENDERER_WEBGL,
  );
  const renderer =
    typeof rendererValue === "string" ? rendererValue.trim() : "";
  if (!renderer || renderer === "null" || renderer === "undefined") {
    return {
      supported: false,
      code: "hardware_unconfirmed",
      renderer: "não confirmado",
      reason:
        "WebGL2 existe, mas o backend de hardware não pôde ser confirmado.",
      limits,
    };
  }
  const normalized = renderer.toLocaleLowerCase();
  if (SOFTWARE_RENDERERS.some((token) => normalized.includes(token))) {
    return {
      supported: false,
      code: "software_renderer",
      renderer,
      reason:
        "O WebGL2 disponível está usando rasterização por software, que não atende ao editor.",
      limits,
    };
  }

  return {
    supported: true,
    renderer,
    reason: "WebGL2 acelerado por hardware confirmado.",
    limits,
  };
}
