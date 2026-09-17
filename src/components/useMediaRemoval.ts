import { useEffect, useMemo, useRef, useState } from "react";
import type { ProjectDialogPort, ProjectDialogState } from "../application/projectDialogPort";
import { createProjectDecisions } from "../application/projectDecision";
import type { EditorProjection } from "../domain/project";
import type { ProjectMutationOutcome, ProjectMutationRunner } from "./useProjectMutationRunner";

export function useMediaRemoval(input: {
  projection: EditorProjection;
  runner: ProjectMutationRunner;
  dialogPort: ProjectDialogPort;
  onProjectionChange(projection: EditorProjection): void;
  onError(message: string): void;
}) {
  const context = useMemo(() => ({ active: true, decisions: createProjectDecisions(input.dialogPort) }),
    [input.projection.state.projectId, input.runner, input.dialogPort]);
  const latest = useRef(input);
  latest.current = input;
  const [activeContext, setActiveContext] = useState<typeof context | null>(null);
  useEffect(() => {
    context.active = true;
    return () => { context.active = false; context.decisions.cancel(); };
  }, [context]);
  async function request(mediaIds: readonly string[]) {
    if (!context.active || context.decisions.busy || mediaIds.length === 0) return;
    setActiveContext(context);
    try {
      const outcome = await context.decisions.run<ProjectMutationOutcome | null>(null, async (decision) => {
        const pending = await latest.current.runner.waitForIdle();
        if (!decision.current || pending?.status === "failed") return null;
        const projection = pending?.status === "completed" ? pending.projection : latest.current.projection;
        const selected = projection.state.album.media.filter((media) => mediaIds.includes(media.id));
        if (selected.length === 0) return null;
        const ids = selected.map((media) => media.id);
        const uses = projection.mediaUsage.filter((usage) => ids.includes(usage.mediaId));
        const state: Extract<ProjectDialogState, { kind: "mediaRemovalConfirmation" }> = {
          kind: "mediaRemovalConfirmation", mediaKind: selected[0].kind, count: selected.length,
          usedCount: uses.filter((usage) => usage.count > 0).length,
          usageCount: uses.reduce((count, usage) => count + usage.count, 0), busy: false,
        };
        let mode: "removeAll" | "keepFrames" = "removeAll";
        if (!(state.usedCount === 0 && (state.mediaKind === "photo" || state.count === 1))) {
          const answer = await decision.ask(state, (action) => action === "cancelMediaRemoval" ? "cancel"
            : action === "removeAllMedia" ? "removeAll"
            : action === "removeMediaKeepFrames" && state.mediaKind === "photo" ? "keepFrames" : undefined);
          if (!answer || answer === "cancel") return null;
          mode = answer;
          if (!await decision.present({ ...state, busy: true })) return null;
        }
        if (!decision.current) return null;
        return latest.current.runner.run((port, current) => {
          if ((current ?? latest.current.projection).state.revision !== projection.state.revision) throw new Error("O Projeto mudou. Revise a seleção antes de remover.");
          return port.apply({ kind: "removeMedia", mediaIds: ids, mode });
        }, { cancelAfterPendingFailure: true });
      });
      if (!context.active) return;
      if (outcome?.status === "completed") latest.current.onProjectionChange(outcome.projection);
      if (outcome?.status === "failed") latest.current.onError(messageFromError(outcome.error));
    } catch (error: unknown) {
      if (context.active) latest.current.onError(messageFromError(error));
    } finally {
      if (context.active) setActiveContext(null);
    }
  }
  return { active: context === activeContext, request };
}

function messageFromError(error: unknown) { return error instanceof Error ? error.message : String(error); }
