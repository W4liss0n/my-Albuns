import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ProjectCorePort } from "../application/projectPorts";
import type { ComposedFrame, EditorProjection, LayoutQueryResult, ProjectIntent } from "../domain/project";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

interface LayoutPanelInput {
  projection: EditorProjection;
  editing: boolean;
  disabled: boolean;
  catalogRevision?: number;
  port: Pick<ProjectCorePort, "queryLayouts" | "previewLayout">;
  runner: ProjectMutationRunner;
  commit(intent: ProjectIntent): Promise<boolean>;
  onError(message: string): void;
}

interface PreparedLayouts {
  scope: object;
  query: LayoutQueryResult;
  previews: ComposedFrame[][];
}

/** The Core owns candidates, mapping and composition; this session only presents them. */
export function useLayoutPanel(input: LayoutPanelInput) {
  const { projection, editing, disabled } = input;
  const latest = useRef(input);
  latest.current = input;
  const projectId = projection.state.projectId;
  const [target, setTarget] = useState<{ projectId: string; sheetId: string } | null>(null);
  const sheetId = target?.projectId === projectId &&
    projection.state.album.sheets.some((sheet) => sheet.id === target.sheetId) ? target.sheetId : null;
  const visible = sheetId !== null && !editing;
  const sheet = projection.state.album.sheets.find((sheet) => sheet.id === sheetId);
  const frameCount = sheet?.frames.length ?? 0;
  const minimumPositionCount = sheet?.frames.filter((frame) => frame.photo !== null).length ?? 0;
  const [requestedPositions, setRequestedPositions] = useState<{ projectId: string; sheetId: string; count: number } | null>(null);
  const request = requestedPositions?.projectId === projectId && requestedPositions.sheetId === sheetId ? requestedPositions : null;
  const positionCount = sheet?.layoutLocked || minimumPositionCount > 30 ? frameCount : Math.max(minimumPositionCount, request?.count ?? frameCount);
  const explicitPositionCount = request && positionCount <= 30 ? positionCount : null;
  const [refresh, setRefresh] = useState(0);
  const scope = useMemo(() => ({ active: false }),
    [projectId, projection.state.revision, projection.composition, sheetId, editing, disabled, input.port.queryLayouts, input.catalogRevision, refresh, positionCount, explicitPositionCount]);
  const [prepared, setPrepared] = useState<PreparedLayouts | null>(null);
  const [hover, setHover] = useState<{ scope: object; index: number } | null>(null);
  const [error, setError] = useState<{ scope: object; message: string } | null>(null);
  const [committing, setCommitting] = useState(false);
  const committingRef = useRef(false);
  // A late older query must never replace a newer handle in the native session.
  const queries = useRef<Promise<unknown>>(Promise.resolve());

  useLayoutEffect(() => {
    scope.active = true;
    return () => { scope.active = false; };
  }, [scope]);

  useEffect(() => {
    if (!visible || disabled || !sheetId) return;
    let active = true;
    const current = () => active && scope.active;
    const task = queries.current.catch(() => undefined).then(async () => {
      const outcome = await latest.current.runner.waitForIdle();
      if (!current() || outcome?.status === "obsolete") return null;
      if (outcome?.status === "failed") throw outcome.error;
      return explicitPositionCount !== null ? latest.current.port.queryLayouts(sheetId, { frameCount: explicitPositionCount, orientation: "horizontal" })
        : latest.current.port.queryLayouts(sheetId);
    });
    queries.current = task;
    void task.then(async (query) => {
      if (!current() || !query || query.projectId !== projectId ||
          query.sheetId !== sheetId || query.revision !== latest.current.projection.state.revision) return;
      const previews = await Promise.all(query.listing.candidates.map((_, candidateIndex) =>
        latest.current.port.previewLayout({ queryId: query.queryId, candidateIndex })));
      if (current()) setPrepared({ scope, query, previews });
    }).catch((cause: unknown) => {
      if (!current()) return;
      const message = cause instanceof Error ? cause.message : String(cause);
      setError({ scope, message });
      latest.current.onError(message);
    });
    return () => { active = false; };
  }, [scope, visible, disabled, sheetId, projectId, explicitPositionCount, frameCount]);

  const data = visible && !disabled && prepared?.scope === scope ? prepared : null;
  // Keep the last thumbnails painted while this same target refreshes. Only
  // current-scope data may preview or commit; another target never reuses it.
  const displayData = visible && prepared?.query.projectId === projectId &&
    prepared.query.sheetId === sheetId ? prepared : null;
  const previewFrames = hover?.scope === scope && data ? data.previews[hover.index] : null;
  const composition = useMemo(() => previewFrames ? {
    ...projection.composition,
    sheets: projection.composition.sheets.map((sheet) => sheet.sheetId === sheetId
      ? { ...sheet, frames: previewFrames } : sheet),
  } : projection.composition, [previewFrames, projection.composition, sheetId]);

  async function commit(intent: ProjectIntent): Promise<boolean> {
    if (!data || disabled || committingRef.current) return false;
    committingRef.current = true;
    setCommitting(true);
    setHover(null);
    try {
      // The captured handle deliberately stays fixed while the shared queue advances.
      // The Core rejects an obsolete patch instead of applying a different preview.
      return await latest.current.commit(intent);
    } finally {
      committingRef.current = false;
      setCommitting(false);
      setRefresh((value) => value + 1);
    }
  }

  return {
    visible, sheetId, composition, committing, positionCount, minimumPositionCount,
    query: data?.query ?? null,
    displayQuery: displayData?.query ?? null,
    previews: displayData?.previews ?? [],
    error: error?.scope === scope ? error.message : null,
    toggle(nextSheetId: string) {
      if (disabled || editing) return;
      setHover(null);
      setTarget(sheetId === nextSheetId ? null : { projectId, sheetId: nextSheetId });
    },
    close() { setHover(null); setTarget(null); },
    refresh() { setHover(null); setRefresh((value) => value + 1); },
    configurePositions(count: number) {
      if (!sheetId || disabled || committingRef.current || sheet?.layoutLocked || !Number.isSafeInteger(count) || count < minimumPositionCount || count > 30) return;
      setHover(null);
      setRequestedPositions({ projectId, sheetId, count });
    },
    preview(index: number) {
      if (data?.previews[index] && !data.query.locked && !committingRef.current) setHover({ scope, index });
    },
    cancelPreview() { setHover(null); },
    apply(index: number) {
      if (!data?.previews[index] || data.query.locked || data.query.listing.candidates[index].layout.definition.positions.length > data.query.frameCount) return Promise.resolve(false);
      return commit({ kind: "applyLayout", selection: { queryId: data.query.queryId, candidateIndex: index } });
    },
    lock(index: number) {
      if (!data?.previews[index] || data.query.locked) return Promise.resolve(false);
      return commit({ kind: "lockLayout", selection: { queryId: data.query.queryId, candidateIndex: index } });
    },
    toggleFavorite(index: number) {
      if (!data?.previews[index] || (data.query.locked && index !== 0)) return Promise.resolve(false);
      return commit({ kind: "toggleLayoutFavorite", selection: { queryId: data.query.queryId, candidateIndex: index } });
    },
    unlock() {
      if (!data?.query.locked) return Promise.resolve(false);
      return commit({ kind: "unlockLayout", sheetId: data.query.sheetId });
    },
  };
}

export type LayoutPanelController = ReturnType<typeof useLayoutPanel>;
