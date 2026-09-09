import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ProjectCorePort } from "../application/projectPorts";
import type { ComposedFrame, CompositionPlan, EditorProjection, PhotoAngleEdit } from "../domain/project";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

interface PhotoAngleEditingInput {
  projection: EditorProjection;
  frameIds: readonly string[];
  disabled: boolean;
  port: ProjectCorePort;
  runner: ProjectMutationRunner;
  commit(edit: PhotoAngleEdit): Promise<EditorProjection | null>;
  onError(message: string): void;
}

interface AnglePreview {
  scope: object;
  base: CompositionPlan;
  frames: ComposedFrame[];
}

export function usePhotoAngleEditing(input: PhotoAngleEditingInput) {
  const { projection, disabled, port } = input;
  const latest = useRef(input);
  latest.current = input;
  const scopeKey = JSON.stringify([projection.state.projectId, [...input.frameIds].sort()]);
  const scope = useMemo(() => ({
    active: false, serial: 0, draft: null as PhotoAngleEdit | null,
  }), [scopeKey, disabled, port]);
  const [visual, setVisual] = useState<AnglePreview | null>(null);
  const [settlement, setSettlement] = useState({ serial: 0, reset: false });
  const [doubleClickTimeMs, setDoubleClickTimeMs] = useState<number | null>(null);

  useLayoutEffect(() => {
    scope.active = true;
    return () => { scope.active = false; scope.serial += 1; scope.draft = null; };
  }, [scope]);

  useEffect(() => {
    let active = true;
    let request = 0;
    const read = () => {
      const serial = ++request;
      setDoubleClickTimeMs(null);
      void port.readPhotoAngleDoubleClickTime().then((milliseconds) => {
        if (!Number.isInteger(milliseconds) || milliseconds <= 0 || milliseconds > 5_000) {
          throw new Error("Não foi possível consultar o intervalo de dois cliques do Windows.");
        }
        if (active && serial === request) setDoubleClickTimeMs(milliseconds);
      }).catch((error: unknown) => {
        if (active && serial === request) latest.current.onError(error instanceof Error ? error.message : String(error));
      });
    };
    read();
    window.addEventListener("focus", read);
    return () => { active = false; window.removeEventListener("focus", read); };
  }, [port, projection.state.projectId]);

  const cancel = useCallback(() => {
    scope.serial += 1;
    scope.draft = null;
    if (scope.active) {
      setVisual(null);
      setSettlement((previous) => ({ serial: previous.serial + 1, reset: true }));
    }
  }, [scope]);

  const preview = useCallback((angleTenths: number) => {
    if (!scope.active || disabled) return;
    const edit = { frameIds: [...latest.current.frameIds], angleTenths };
    scope.draft = edit;
    const serial = ++scope.serial;
    const current = () => scope.active && scope.serial === serial;
    // Preview is read-only, but it must observe any preceding queued mutation.
    void latest.current.runner.waitForIdle().then(async (outcome) => {
      if (!current()) return;
      if (outcome?.status === "failed") throw outcome.error;
      const base = latest.current.projection.composition;
      const frames = await port.previewPhotoAngle(edit);
      if (current() && base === latest.current.projection.composition) {
        setVisual({ scope, base, frames });
      }
    }).catch((error: unknown) => {
      if (!current()) return;
      cancel();
      latest.current.onError(error instanceof Error ? error.message : String(error));
    });
  }, [scope, disabled, port, cancel]);

  useEffect(() => {
    // A source observation or an adjacent edit changes the authoritative composition.
    // Re-request the same draft instead of applying a plan calculated against old state.
    if (scope.draft) preview(scope.draft.angleTenths);
  }, [projection.composition, scope, preview]);

  const commit = useCallback(async (angleTenths?: number) => {
    if (!scope.active || disabled) return false;
    const edit = angleTenths === undefined ? scope.draft : {
      frameIds: [...latest.current.frameIds], angleTenths,
    };
    if (!edit) return false;
    scope.draft = null;
    const serial = ++scope.serial;
    setSettlement((previous) => ({ serial: previous.serial + 1, reset: false }));
    try {
      // Enqueue synchronously, before the next Save, Undo or Photo command.
      const completed = (await latest.current.commit(edit)) !== null;
      if (!completed && scope.active && scope.serial === serial) {
        setSettlement((previous) => ({ serial: previous.serial + 1, reset: true }));
      }
      return completed;
    } finally {
      if (scope.active && scope.serial === serial) setVisual(null);
    }
  }, [scope, disabled]);

  const composition = useMemo(() => {
    if (!visual || disabled || visual.scope !== scope || visual.base !== projection.composition) {
      return projection.composition;
    }
    const frames = new Map(visual.frames.map((frame) => [frame.frameId, frame]));
    return { ...projection.composition, sheets: projection.composition.sheets.map((sheet) => ({
      ...sheet, frames: sheet.frames.map((frame) => frames.get(frame.frameId) ?? frame),
    })) };
  }, [visual, disabled, scope, projection.composition]);

  return { scopeKey, preview, commit, cancel, composition, doubleClickTimeMs, settlement };
}
