import type {
  AlbumInformation,
  EditorProjection,
  ProjectIntent,
  ProjectedBackgroundContent,
  ProjectedFrameBorder,
  ProjectedOverlayContent,
  ProjectedVisualDefaults,
} from "../domain/project";
import { applyScopedValue, type ScopedValue } from "./scopedValues";

type SettingsDelta<Value> = Partial<Value>;

type ScopedDelta<Value> =
  | { kind: "none" }
  | { kind: "both"; value: Value }
  | { kind: "sides"; left?: Value; right?: Value }
  | { kind: "perSide" };

interface AlbumDesignDelta {
  background: ScopedDelta<ProjectedBackgroundContent>;
  overlay: ScopedDelta<ProjectedOverlayContent | null>;
  frameBorder?: ProjectedFrameBorder;
}

/**
 * A semantic draft keeps the user transition separate from the complete
 * snapshot required by the Rust intent. The delta is always replayed over the
 * latest authoritative projection when the queued mutation actually runs.
 */
export interface ProjectSettingsDraft<Value, Delta = SettingsDelta<Value>> {
  readonly baselineRevision: number;
  readonly baseline: Readonly<Value>;
  readonly value: Readonly<Value>;
  readonly delta: Readonly<Delta>;
  readonly changed: boolean;
  equals(value: Value): boolean;
  transition(value: Value): ProjectSettingsDraft<Value, Delta>;
  /** Carries the same semantic delta onto a newer authoritative baseline. */
  rebase(
    baselineRevision: number,
    baseline: Value,
  ): ProjectSettingsDraft<Value, Delta>;
  materializeAgainst(
    latestProjection: EditorProjection,
  ): MaterializedProjectSettings<Value>;
  materialize(latestProjection: EditorProjection): ProjectIntent;
}

export interface MaterializedProjectSettings<Value> {
  readonly baselineRevision: number;
  readonly baseline: Readonly<Value>;
  readonly value: Readonly<Value>;
  /** Whether applying `intent` would change this authoritative baseline. */
  readonly changed: boolean;
  readonly intent: ProjectIntent;
}

export type AlbumInformationProjectDraft =
  ProjectSettingsDraft<AlbumInformation>;
export type AlbumDesignProjectDraft =
  ProjectSettingsDraft<ProjectedVisualDefaults, AlbumDesignDelta>;

export function createAlbumInformationProjectDraft(
  baselineRevision: number,
  baseline: AlbumInformation,
): AlbumInformationProjectDraft {
  return createProjectSettingsDraft(
    baselineRevision,
    baseline,
    albumInformationFromProjection,
    (information) => ({ kind: "setAlbumInformation", information }),
    changedFields,
    (latest, delta) => ({ ...latest, ...delta }),
    (delta) => Object.keys(delta).length > 0,
  );
}

export function createAlbumDesignProjectDraft(
  baselineRevision: number,
  baseline: ProjectedVisualDefaults,
): AlbumDesignProjectDraft {
  return createProjectSettingsDraft(
    baselineRevision,
    baseline,
    (projection) => projection.state.album.visualDefaults,
    (visualDefaults) => ({ kind: "setVisualDefaults", visualDefaults }),
    albumDesignDelta,
    applyAlbumDesignDelta,
    albumDesignDeltaChanged,
  );
}

/**
 * Turns legacy complete-snapshot intents into semantic drafts at the call
 * seam. Delta intents remain unchanged because their meaning is already
 * relative to the state at execution.
 */
export function materializeProjectIntent(
  intent: ProjectIntent,
  capturedProjection: EditorProjection,
  latestProjection: EditorProjection,
): ProjectIntent {
  switch (intent.kind) {
    case "setAlbumInformation":
      return createAlbumInformationProjectDraft(
        capturedProjection.state.revision,
        albumInformationFromProjection(capturedProjection),
      )
        .transition(intent.information)
        .materialize(latestProjection);
    case "setVisualDefaults":
      return createAlbumDesignProjectDraft(
        capturedProjection.state.revision,
        capturedProjection.state.album.visualDefaults,
      )
        .transition(intent.visualDefaults)
        .materialize(latestProjection);
    default:
      return intent;
  }
}

function createProjectSettingsDraft<Value, Delta>(
  baselineRevision: number,
  baseline: Value,
  readLatestValue: (projection: EditorProjection) => Value,
  createIntent: (value: Value) => ProjectIntent,
  calculateDelta: (baseline: Value, value: Value) => Delta,
  applyDelta: (latest: Value, delta: Delta) => Value,
  deltaChanged: (delta: Delta) => boolean,
  value: Value = baseline,
): ProjectSettingsDraft<Value, Delta> {
  const delta = calculateDelta(baseline, value);
  const materializeAgainst = (
    latestProjection: EditorProjection,
  ): MaterializedProjectSettings<Value> => {
    const latest = readLatestValue(latestProjection);
    const materialized = applyDelta(latest, delta);
    return {
      baselineRevision: latestProjection.state.revision,
      baseline: latest,
      value: materialized,
      changed: !structuralEquals(latest, materialized),
      intent: createIntent(materialized),
    };
  };
  return {
    baselineRevision,
    baseline,
    value,
    delta,
    changed: deltaChanged(delta),
    equals(candidate) {
      return structuralEquals(value, candidate);
    },
    transition(candidate) {
      return createProjectSettingsDraft(
        baselineRevision,
        baseline,
        readLatestValue,
        createIntent,
        calculateDelta,
        applyDelta,
        deltaChanged,
        candidate,
      );
    },
    rebase(nextBaselineRevision, nextBaseline) {
      return createProjectSettingsDraft(
        nextBaselineRevision,
        nextBaseline,
        readLatestValue,
        createIntent,
        calculateDelta,
        applyDelta,
        deltaChanged,
        applyDelta(nextBaseline, delta),
      );
    },
    materializeAgainst,
    materialize(latestProjection) {
      return materializeAgainst(latestProjection).intent;
    },
  };
}

function changedFields<Value>(
  baseline: Value,
  value: Value,
): SettingsDelta<Value> {
  const delta: SettingsDelta<Value> = {};
  for (const key of Object.keys(value as object) as Array<keyof Value>) {
    if (!structuralEquals(baseline[key], value[key])) {
      delta[key] = value[key];
    }
  }
  return delta;
}

function structuralEquals(left: unknown, right: unknown) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function albumDesignDelta(
  baseline: ProjectedVisualDefaults,
  value: ProjectedVisualDefaults,
): AlbumDesignDelta {
  return {
    background: scopedDelta(baseline.background, value.background),
    overlay: scopedDelta(baseline.overlay, value.overlay),
    ...(structuralEquals(baseline.frameBorder, value.frameBorder)
      ? {}
      : { frameBorder: value.frameBorder }),
  };
}

function albumDesignDeltaChanged(delta: AlbumDesignDelta) {
  return (
    delta.background.kind !== "none" ||
    delta.overlay.kind !== "none" ||
    delta.frameBorder !== undefined
  );
}

function applyAlbumDesignDelta(
  latest: ProjectedVisualDefaults,
  delta: AlbumDesignDelta,
): ProjectedVisualDefaults {
  return {
    background: applyScopedDelta(latest.background, delta.background),
    overlay: applyScopedDelta(latest.overlay, delta.overlay),
    frameBorder: delta.frameBorder ?? latest.frameBorder,
  };
}

function scopedDelta<Value>(
  baseline: ScopedValue<Value>,
  value: ScopedValue<Value>,
): ScopedDelta<Value> {
  if (structuralEquals(baseline, value)) return { kind: "none" };
  if (value.scope === "bothSides") {
    return { kind: "both", value: value.both };
  }
  const leftChanged = !structuralEquals(
    valueAtSide(baseline, "left"),
    value.left,
  );
  const rightChanged = !structuralEquals(
    valueAtSide(baseline, "right"),
    value.right,
  );
  if (!leftChanged && !rightChanged) {
    return { kind: "perSide" };
  }
  return {
    kind: "sides",
    ...(leftChanged ? { left: value.left } : {}),
    ...(rightChanged ? { right: value.right } : {}),
  };
}

function applyScopedDelta<Value>(
  latest: ScopedValue<Value>,
  delta: ScopedDelta<Value>,
): ScopedValue<Value> {
  switch (delta.kind) {
    case "none":
      return latest;
    case "both":
      return applyScopedValue(latest, "both", delta.value);
    case "perSide":
      return latest.scope === "perSide"
        ? latest
        : { scope: "perSide", left: latest.both, right: latest.both };
    case "sides": {
      const withLeft =
        delta.left === undefined
          ? latest
          : applyScopedValue(latest, "left", delta.left);
      return delta.right === undefined
        ? withLeft
        : applyScopedValue(withLeft, "right", delta.right);
    }
  }
}

function valueAtSide<Value>(
  value: ScopedValue<Value>,
  side: "left" | "right",
) {
  return value.scope === "bothSides" ? value.both : value[side];
}

function albumInformationFromProjection(
  projection: EditorProjection,
): AlbumInformation {
  const { document, album } = projection.state;
  return {
    displayUnit: document.displayUnit,
    sheetWidthUm: document.sheetWidthUm,
    sheetHeightUm: document.sheetHeightUm,
    dpi: document.dpi,
    bleedUm: document.bleedUm,
    safetyUm: document.safetyUm,
    firstSheet: endSheetFormat(album.sheets[0]),
    lastSheet: endSheetFormat(album.sheets[album.sheets.length - 1]),
  };
}

function endSheetFormat(
  sheet: EditorProjection["state"]["album"]["sheets"][number] | undefined,
): AlbumInformation["firstSheet"] {
  return sheet?.activeSides === "both" ? "double" : "singlePage";
}
