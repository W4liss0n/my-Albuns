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
        "Não foi possível ativar a aceleração gráfica necessária para abrir o editor.",
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
        "Não foi possível verificar a capacidade gráfica necessária para abrir o editor.",
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
        "Não foi possível confirmar a aceleração gráfica necessária para abrir o editor.",
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
        "Não foi possível confirmar a aceleração gráfica necessária para abrir o editor.",
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
        "O editor precisa de aceleração gráfica, que não está disponível nesta tentativa.",
      limits,
    };
  }

  return {
    supported: true,
    renderer,
    reason: "A aceleração gráfica está disponível.",
    limits,
  };
}
