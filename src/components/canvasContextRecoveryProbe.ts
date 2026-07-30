import type {
  CanvasGraphicsMeasurement,
  CanvasTestedTexture,
} from "../application/topologyBenchmark";

const CONTEXT_EVENT_TIMEOUT_MS = 10_000;

export interface CanvasContextRestoredFrame {
  renderedAt: number;
  textureBacked: boolean;
  decorativeTextureBacked: boolean;
  testedTexture: CanvasTestedTexture;
}

export interface CanvasContextRecoveryProbeTarget {
  canvas: HTMLCanvasElement;
  renderAfterRestore(): Promise<CanvasContextRestoredFrame>;
}

export interface CanvasContextRecoveryProbeClock {
  now(): number;
}

const browserClock: CanvasContextRecoveryProbeClock = {
  now: () => performance.now(),
};

export async function runCanvasContextRecoveryProbe(
  target: CanvasContextRecoveryProbeTarget,
  clock: CanvasContextRecoveryProbeClock = browserClock,
  signal?: AbortSignal,
): Promise<CanvasGraphicsMeasurement> {
  throwIfAborted(signal);
  const gl = target.canvas.getContext("webgl2");
  if (!gl) {
    throw new Error("O Canvas real não disponibilizou um contexto WebGL2.");
  }
  const extension = gl.getExtension("WEBGL_lose_context");
  if (!extension) {
    throw new Error(
      "WEBGL_lose_context não está disponível no Canvas real.",
    );
  }

  const maxTextureSizePx = readPositiveInteger(
    gl,
    gl.MAX_TEXTURE_SIZE,
    "MAX_TEXTURE_SIZE",
  );
  const maxRenderbufferSizePx = readPositiveInteger(
    gl,
    gl.MAX_RENDERBUFFER_SIZE,
    "MAX_RENDERBUFFER_SIZE",
  );
  const maxTextureImageUnits = readPositiveInteger(
    gl,
    gl.MAX_TEXTURE_IMAGE_UNITS,
    "MAX_TEXTURE_IMAGE_UNITS",
  );

  let contextLossObserved = false;
  let contextRestored = false;
  try {
    const lostEvent = waitForContextEvent(
      target.canvas,
      "webglcontextlost",
      signal,
      true,
    );
    extension.loseContext();
    await lostEvent;
    contextLossObserved = true;
    throwIfAborted(signal);
    if (!gl.isContextLost()) {
      throw new Error(
        "O evento de perda ocorreu sem o contexto WebGL2 entrar no estado perdido.",
      );
    }
    const lostAt = clock.now();

    // WebView2 only accepts an explicit restoration after it has finished
    // dispatching the context-loss task to every WebGL consumer, including Pixi.
    await nextTask();
    throwIfAborted(signal);
    const restoredEvent = waitForContextEvent(
      target.canvas,
      "webglcontextrestored",
      signal,
    );
    extension.restoreContext();
    await restoredEvent;
    throwIfAborted(signal);
    if (gl.isContextLost()) {
      throw new Error(
        "O contexto WebGL2 continuou perdido após o evento de restauração.",
      );
    }
    contextRestored = true;
    const restoredAt = clock.now();
    const restoredFrame = await target.renderAfterRestore();
    throwIfAborted(signal);
    gl.finish();
    const glError = gl.getError();

    return {
      webGlVersion: 2,
      maxTextureSizePx,
      maxRenderbufferSizePx,
      maxTextureImageUnits,
      testedTexture: restoredFrame.testedTexture,
      contextRecovery: {
        mechanism: "webgl_lose_context",
        contextLost: true,
        contextRestored: true,
        recoveryDurationMs: round(
          Math.max(0, restoredFrame.renderedAt - lostAt),
        ),
        restoredFrameLatencyMs: round(
          Math.max(0, restoredFrame.renderedAt - restoredAt),
        ),
        glError,
        textureBacked: restoredFrame.textureBacked,
        decorativeTextureBacked:
          restoredFrame.decorativeTextureBacked,
      },
    };
  } finally {
    if (
      !contextRestored &&
      (contextLossObserved || isContextLost(gl))
    ) {
      // WebView2 applies the same task-boundary requirement to emergency
      // restoration after cancellation or another probe failure.
      await nextTask();
      try {
        extension.restoreContext();
      } catch {
        // Recovery is best-effort here so the original probe failure is kept.
      }
    }
  }
}

function nextTask() {
  return new Promise<void>((resolve) => {
    globalThis.setTimeout(resolve, 0);
  });
}

function readPositiveInteger(
  gl: WebGL2RenderingContext,
  parameter: number,
  label: string,
) {
  const value = gl.getParameter(parameter);
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`O limite gráfico ${label} é inválido.`);
  }
  return value as number;
}

function waitForContextEvent(
  canvas: HTMLCanvasElement,
  type: "webglcontextlost" | "webglcontextrestored",
  signal?: AbortSignal,
  preventDefault = false,
) {
  throwIfAborted(signal);
  return new Promise<Event>((resolve, reject) => {
    const timeout = globalThis.setTimeout(() => {
      cleanup();
      reject(
        new Error(
          `O Canvas não emitiu ${type} dentro do tempo de segurança.`,
        ),
      );
    }, CONTEXT_EVENT_TIMEOUT_MS);
    const handleEvent = (event: Event) => {
      if (preventDefault) event.preventDefault();
      cleanup();
      resolve(event);
    };
    const handleAbort = () => {
      cleanup();
      reject(
        new DOMException(
          "O diagnóstico de recuperação do Canvas foi cancelado.",
          "AbortError",
        ),
      );
    };
    const cleanup = () => {
      globalThis.clearTimeout(timeout);
      canvas.removeEventListener(type, handleEvent);
      signal?.removeEventListener("abort", handleAbort);
    };

    canvas.addEventListener(type, handleEvent, { once: true });
    signal?.addEventListener("abort", handleAbort, { once: true });
  });
}

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new DOMException(
      "O diagnóstico de recuperação do Canvas foi cancelado.",
      "AbortError",
    );
  }
}

function isContextLost(gl: WebGL2RenderingContext) {
  try {
    return gl.isContextLost();
  } catch {
    return false;
  }
}

function round(value: number) {
  return Number(value.toFixed(3));
}
