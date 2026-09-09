import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ComposedFrame, CompositionPlan, EditorProjection } from "../domain/project";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

export interface FrameCompositionDraftInput<Value> {
  projection: EditorProjection;
  frameIds: readonly string[];
  disabled: boolean;
  session: object;
  runner: ProjectMutationRunner;
  propertyKey?(value: Value): string;
  resolve(frameIds: string[], value: Value): Promise<ComposedFrame[]>;
  commit(frameIds: string[], value: Value): Promise<EditorProjection | null>;
  onError(message: string): void;
}

interface FramePreview {
  scope: object;
  base: CompositionPlan;
  frames: ComposedFrame[];
}

/** One reversible property draft, resolved by the Core against the authoritative queue. */
export function useFrameCompositionDraft<Value>(input: FrameCompositionDraftInput<Value>) {
  const { projection, disabled, session } = input;
  const latest = useRef(input);
  latest.current = input;
  const scopeKey = JSON.stringify([projection.state.projectId, [...input.frameIds].sort()]);
  const scope = useMemo(() => ({ active: false, serial: 0,
    draft: null as { frameIds: string[]; value: Value } | null,
  }), [scopeKey, disabled, session]);
  const [visual, setVisual] = useState<FramePreview | null>(null);
  const [settlement, setSettlement] = useState({ serial: 0, reset: false });

  useLayoutEffect(() => {
    scope.active = true;
    return () => { scope.active = false; scope.serial += 1; scope.draft = null; };
  }, [scope]);

  const cancel = useCallback(() => {
    scope.serial += 1;
    scope.draft = null;
    if (scope.active) {
      setVisual(null);
      setSettlement((previous) => ({ serial: previous.serial + 1, reset: true }));
    }
  }, [scope]);

  const commit = useCallback(async function commitDraft(value?: Value): Promise<boolean> {
    if (!scope.active || disabled) return false;
    const propertyKey = latest.current.propertyKey;
    if (value !== undefined && scope.draft && propertyKey &&
        propertyKey(scope.draft.value) !== propertyKey(value)) void commitDraft();
    const edit = value === undefined ? scope.draft : { frameIds: [...latest.current.frameIds], value };
    if (!edit) return false;
    scope.draft = null;
    const serial = ++scope.serial;
    setSettlement((previous) => ({ serial: previous.serial + 1, reset: false }));
    try {
      // Enqueue synchronously so Save, Undo and adjacent properties follow this edit.
      const completed = (await latest.current.commit(edit.frameIds, edit.value)) !== null;
      if (!completed && scope.active && scope.serial === serial) {
        setSettlement((previous) => ({ serial: previous.serial + 1, reset: true }));
      }
      return completed;
    } finally {
      if (scope.active && scope.serial === serial) setVisual(null);
    }
  }, [scope, disabled]);

  const preview = useCallback((value: Value) => {
    if (!scope.active || disabled) return;
    const propertyKey = latest.current.propertyKey;
    if (scope.draft && propertyKey && propertyKey(scope.draft.value) !== propertyKey(value)) void commit();
    const edit = { frameIds: [...latest.current.frameIds], value };
    scope.draft = edit;
    const serial = ++scope.serial;
    const current = () => scope.active && scope.serial === serial;
    void latest.current.runner.waitForIdle().then(async (outcome) => {
      if (!current()) return;
      if (outcome?.status === "failed") throw outcome.error;
      const base = latest.current.projection.composition;
      const frames = await latest.current.resolve(edit.frameIds, edit.value);
      if (current() && base === latest.current.projection.composition) setVisual({ scope, base, frames });
    }).catch((error: unknown) => {
      if (!current()) return;
      cancel();
      latest.current.onError(error instanceof Error ? error.message : String(error));
    });
  }, [scope, disabled, commit, cancel]);

  useEffect(() => {
    // Re-resolve after source observations or edits; never reuse an obsolete plan.
    if (scope.draft) preview(scope.draft.value);
  }, [projection.composition, scope, preview]);

  const composition = useMemo(() => {
    if (!visual || disabled || visual.scope !== scope || visual.base !== projection.composition) return projection.composition;
    const frames = new Map(visual.frames.map((frame) => [frame.frameId, frame]));
    return { ...projection.composition, sheets: projection.composition.sheets.map((sheet) => ({
      ...sheet, frames: sheet.frames.map((frame) => frames.get(frame.frameId) ?? frame),
    })) };
  }, [visual, disabled, scope, projection.composition]);

  return { scopeKey, preview, commit, cancel, composition, settlement };
}
