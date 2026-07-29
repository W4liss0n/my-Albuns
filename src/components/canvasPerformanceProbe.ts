export interface CanvasPerformanceProbeConfig {
  warmupFrames: number;
  panFrames: number;
  zoomFrames: number;
}

export interface CanvasPerformanceTarget {
  frameId: string;
  textureBacked: boolean;
  previewPan(amount: number): void;
  previewZoom(amount: number): void;
  reset(): void;
}

export interface CanvasPerformanceClock {
  now(): number;
  nextFrame(): Promise<number>;
}

export interface FrameTimingSummary {
  sampleCount: number;
  durationMs: number;
  firstFrameLatencyMs: number;
  meanFrameMs: number;
  p50FrameMs: number;
  p95FrameMs: number;
  p99FrameMs: number;
  maxFrameMs: number;
  framesOver16Ms: number;
  framesOver33Ms: number;
}

export interface CanvasPerformanceMeasurement {
  frameId: string;
  textureBacked: boolean;
  pan: FrameTimingSummary;
  zoom: FrameTimingSummary;
}

const browserFrameClock: CanvasPerformanceClock = {
  now: () => performance.now(),
  nextFrame: () =>
    new Promise<number>((resolve) => {
      requestAnimationFrame(resolve);
    }),
};

export async function runCanvasPerformanceProbe(
  config: CanvasPerformanceProbeConfig,
  target: CanvasPerformanceTarget,
  clock: CanvasPerformanceClock = browserFrameClock,
  signal?: AbortSignal,
): Promise<CanvasPerformanceMeasurement> {
  validateConfig(config);
  if (!target.textureBacked) {
    throw new Error(
      "O probe do Canvas exige uma Foto materializada com textura real.",
    );
  }

  try {
    for (let index = 0; index < config.warmupFrames; index += 1) {
      throwIfAborted(signal);
      target.previewPan(wave(index, config.warmupFrames));
      await clock.nextFrame();
    }

    const pan = await measureFrames(
      config.panFrames,
      (index) => target.previewPan(wave(index, config.panFrames)),
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
      clock,
      signal,
    );

    return {
      frameId: target.frameId,
      textureBacked: target.textureBacked,
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
  clock: CanvasPerformanceClock,
  signal?: AbortSignal,
) {
  const latencies: number[] = [];
  for (let index = 0; index < sampleCount; index += 1) {
    throwIfAborted(signal);
    apply(index);
    const requestedAt = clock.now();
    const presentedAt = await clock.nextFrame();
    throwIfAborted(signal);
    latencies.push(Math.max(0, presentedAt - requestedAt));
  }
  return summarizeFrameTimings(latencies);
}

function summarizeFrameTimings(
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
