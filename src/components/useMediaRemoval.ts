import { useEffect, useMemo, useRef, useState } from "react";
import type { ProjectDialogPort, ProjectDialogSession, ProjectDialogState } from "../application/projectDialogPort";
import type { EditorProjection } from "../domain/project";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

export function useMediaRemoval(input: {
  projection: EditorProjection;
  runner: ProjectMutationRunner;
  dialogPort: ProjectDialogPort;
  onProjectionChange(projection: EditorProjection): void;
  onError(message: string): void;
}) {
  const context = useMemo(() => ({ active: true, busy: false, dialog: null as ProjectDialogSession | null }), [input.projection.state.projectId, input.runner, input.dialogPort]);
  const latest = useRef(input);
  latest.current = input;
  const [activeContext, setActiveContext] = useState<typeof context | null>(null);
  useEffect(() => {
    context.active = true;
    return () => { context.active = false; void context.dialog?.dismiss().catch(() => undefined); };
  }, [context]);
  async function request(mediaIds: readonly string[]) {
    if (!context.active || context.busy || mediaIds.length === 0) return;
    context.busy = true;
    setActiveContext(context);
    const finish = async () => {
      const dialog = context.dialog;
      context.dialog = null;
      await dialog?.dismiss().catch(() => undefined);
      context.busy = false;
      if (context.active) setActiveContext(null);
    };
    const fail = async (error: unknown) => {
      await finish();
      if (context.active) latest.current.onError(error instanceof Error ? error.message : String(error));
    };
    try {
      const pending = await latest.current.runner.waitForIdle();
      if (!context.active) return;
      if (pending?.status === "failed") { await finish(); return; }
      const projection = pending?.status === "completed" ? pending.projection : latest.current.projection;
      const selected = projection.state.album.media.filter((media) => mediaIds.includes(media.id));
      if (selected.length === 0) { await finish(); return; }
      const ids = selected.map((media) => media.id);
      const uses = projection.mediaUsage.filter((usage) => ids.includes(usage.mediaId));
      const state: Extract<ProjectDialogState, { kind: "mediaRemovalConfirmation" }> = {
        kind: "mediaRemovalConfirmation", mediaKind: selected[0].kind, count: selected.length,
        usedCount: uses.filter((usage) => usage.count > 0).length,
        usageCount: uses.reduce((count, usage) => count + usage.count, 0), busy: false,
      };
      const confirm = async (mode: "removeAll" | "keepFrames") => {
        try {
          await context.dialog?.present({ ...state, busy: true });
          if (!context.active) return;
          const outcome = await latest.current.runner.run((port, current) => {
            if ((current ?? latest.current.projection).state.revision !== projection.state.revision) throw new Error("O Projeto mudou. Revise a seleção antes de remover.");
            return port.apply({ kind: "removeMedia", mediaIds: ids, mode });
          }, { cancelAfterPendingFailure: true });
          await finish();
          if (!context.active) return;
          if (outcome.status === "completed") latest.current.onProjectionChange(outcome.projection);
          if (outcome.status === "failed") latest.current.onError(outcome.error instanceof Error ? outcome.error.message : String(outcome.error));
        } catch (error: unknown) { await fail(error); }
      };
      if (state.usedCount === 0 && (state.mediaKind === "photo" || state.count === 1)) { await confirm("removeAll"); return; }
      let deciding = true;
      context.dialog = latest.current.dialogPort.acquire((action) => {
        if (!context.active || !deciding) return;
        if (action === "cancelMediaRemoval") { deciding = false; void finish(); }
        if (action === "removeAllMedia" || action === "removeMediaKeepFrames" && state.mediaKind === "photo") {
          deciding = false; void confirm(action === "removeAllMedia" ? "removeAll" : "keepFrames");
        }
      });
      await context.dialog.present(state);
    } catch (error: unknown) { await fail(error); }
  }
  return { active: context === activeContext, request };
}
