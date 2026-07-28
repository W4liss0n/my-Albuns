export interface GraphicsDiagnostic {
  supported: boolean;
  renderer: string;
  reason: string;
}

const SOFTWARE_RENDERERS = [
  "swiftshader",
  "llvmpipe",
  "software",
  "microsoft basic render",
];

export function probeGraphics(): GraphicsDiagnostic {
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("webgl2", {
    failIfMajorPerformanceCaveat: true,
    powerPreference: "high-performance",
  });

  if (!context) {
    return {
      supported: false,
      renderer: "indisponível",
      reason:
        "Não foi possível criar um contexto WebGL2 acelerado por hardware.",
    };
  }

  const rendererExtension = context.getExtension("WEBGL_debug_renderer_info");
  if (!rendererExtension) {
    return {
      supported: false,
      renderer: "não confirmado",
      reason:
        "WebGL2 existe, mas o backend de hardware não pôde ser confirmado.",
    };
  }

  const renderer = String(
    context.getParameter(rendererExtension.UNMASKED_RENDERER_WEBGL),
  );
  const normalized = renderer.toLocaleLowerCase();
  if (SOFTWARE_RENDERERS.some((token) => normalized.includes(token))) {
    return {
      supported: false,
      renderer,
      reason:
        "O WebGL2 disponível está usando rasterização por software, que não atende ao editor.",
    };
  }

  return {
    supported: true,
    renderer,
    reason: "WebGL2 acelerado por hardware confirmado.",
  };
}
