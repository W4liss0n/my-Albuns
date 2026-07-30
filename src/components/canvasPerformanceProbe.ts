import type {
  CanvasInteractionPerformanceMeasurement,
  CanvasTestedTexture,
  FrameTimingSummary,
} from "../application/topologyBenchmark";

export interface CanvasPerformanceProbeConfig {
  warmupFrames: number;
  panFrames: number;
  zoomFrames: number;
}

export interface CanvasPerformanceTarget {
  frameId: string;
  textureBacked: boolean;
  decorativeMediaId: string;
  decorativeTextureBacked: boolean;
  testedTexture: CanvasTestedTexture;
  previewPan(amount: number): void;
  previewZoom(amount: number): void;
  nextRenderedFrame(): Promise<number>;
  reset(): void;
}

export type CanvasPerformanceTargetState =
  | { status: "pending" }
  | {
      status: "failed";
      reason:
        | "texture_unavailable"
        | "decorative_texture_unavailable";
    }
  | { status: "ready"; target: CanvasPerformanceTarget };

export interface CanvasPerformanceClock {
  now(): number;
}

const browserFrameClock: CanvasPerformanceClock = {
  now: () => performance.now(),
};

export async function runCanvasPerformanceProbe(
  config: CanvasPerformanceProbeConfig,
  target: CanvasPerformanceTarget,
  clock: CanvasPerformanceClock = browserFrameClock,
  signal?: AbortSignal,
): Promise<CanvasInteractionPerformanceMeasurement> {
  validateConfig(config);
  if (!target.textureBacked) {
    throw new Error(
      "O probe do Canvas exige uma Foto materializada com textura real.",
    );
  }
  if (
    !target.decorativeTextureBacked ||
    target.decorativeMediaId.length === 0
  ) {
    throw new Error(
      "O probe do Canvas exige um Decorativo materializado com textura real.",
    );
  }

  try {
    for (let index = 0; index < config.warmupFrames; index += 1) {
      throwIfAborted(signal);
      target.previewPan(wave(index, config.warmupFrames));
      await target.nextRenderedFrame();
    }

    const pan = await measureFrames(
      config.panFrames,
      (index) => target.previewPan(wave(index, config.panFrames)),
      target,
      clock,
      signal,
    );
    target.reset();
    const zoom = await measureFrames(
      config.zoomFrames,
      (index) =>
        target.previewZoom(
          0.5 + wave(index, config.zoomFrames) * 0.5,
        ),
      target,
      clock,
      signal,
    );

    return {
      frameId: target.frameId,
      textureBacked: target.textureBacked,
      decorativeMediaId: target.decorativeMediaId,
      decorativeTextureBacked: target.decorativeTextureBacked,
      pan,
      zoom,
    };
  } finally {
    target.reset();
  }
}

async function measureFrames(
  sampleCount: number,
  apply: (index: number) => void,
  target: CanvasPerformanceTarget,
  clock: CanvasPerformanceClock,
  signal?: AbortSignal,
) {
  const latencies: number[] = [];
  for (let index = 0; index < sampleCount; index += 1) {
    throwIfAborted(signal);
    const requestedAt = clock.now();
    apply(index);
    const presentedAt = await target.nextRenderedFrame();
    throwIfAborted(signal);
    latencies.push(Math.max(0, presentedAt - requestedAt));
  }
  return summarizeFrameTimings(latencies);
}

export function summarizeFrameTimings(
  latencies: readonly number[],
): FrameTimingSummary {
  const ordered = [...latencies].sort((left, right) => left - right);
  const duration = latencies.reduce((total, value) => total + value, 0);
  return {
    sampleCount: latencies.length,
    durationMs: round(duration),
    firstFrameLatencyMs: round(latencies[0]),
    meanFrameMs: round(duration / latencies.length),
    p50FrameMs: round(percentile(ordered, 0.5)),
    p95FrameMs: round(percentile(ordered, 0.95)),
    p99FrameMs: round(percentile(ordered, 0.99)),
    maxFrameMs: round(ordered[ordered.length - 1]),
    framesOver16Ms: latencies.filter(
      (value) => value > 1000 / 60,
    ).length,
    framesOver33Ms: latencies.filter(
      (value) => value > 1000 / 30,
    ).length,
  };
}

function percentile(ordered: readonly number[], value: number) {
  const index = Math.max(
    0,
    Math.min(ordered.length - 1, Math.ceil(value * ordered.length) - 1),
  );
  return ordered[index];
}

function wave(index: number, count: number) {
  return Math.sin(((index + 0.25) / count) * Math.PI * 2);
}

function validateConfig(config: CanvasPerformanceProbeConfig) {
  for (const [name, value] of Object.entries(config)) {
    if (!Number.isInteger(value) || value < 1 || value > 600) {
      throw new Error(`Configuração inválida do probe do Canvas: ${name}.`);
    }
  }
}

function round(value: number) {
  return Number(value.toFixed(3));
}

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new DOMException("O probe do Canvas foi cancelado.", "AbortError");
  }
}
