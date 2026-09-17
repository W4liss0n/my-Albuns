import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ProjectDialogPort } from "../application/projectDialogPort";
import { createProjectDecisions } from "../application/projectDecision";
import type { ProjectCorePort } from "../application/projectPorts";
import type { CustomLayoutId, EditorProjection, SaveCustomLayoutResult } from "../domain/project";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

interface LayoutCatalogInput {
  projection: EditorProjection;
  runner: ProjectMutationRunner;
  dialogPort: ProjectDialogPort;
  port: Pick<ProjectCorePort, "refreshLayoutCatalog">;
  onError(message: string): void;
}

/** Global catalog writes share the Project queue but never publish a Project revision. */
export function useLayoutCatalog(input: LayoutCatalogInput) {
  const latest = useRef(input);
  latest.current = input;
  const context = useMemo(() => ({ active: false, busy: false, decisions: createProjectDecisions(input.dialogPort) }),
    [input.projection.state.projectId, input.runner, input.dialogPort, input.port]);
  const [revision, setRevision] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{ message: string } | null>(null);
  const noticeContext = useRef(0);
  const dismissNotice = useCallback(() => {
    noticeContext.current += 1;
    setNotice(null);
  }, []);
  const [revealId, setRevealId] = useState<CustomLayoutId | null>(null);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(dismissNotice, 4000);
    return () => window.clearTimeout(timer);
  }, [notice, dismissNotice]);

  useLayoutEffect(() => {
    context.active = true;
    setBusy(false);
    setNotice(null);
    setRevealId(null);
    return () => {
      context.active = false;
      context.decisions.cancel();
    };
  }, [context]);

  async function refresh() {
    try {
      await input.runner.waitForIdle();
      if (!context.active) return false;
      const observed = await input.port.refreshLayoutCatalog();
      if (context.active) setRevision((previous) => Math.max(previous, observed));
      return context.active;
    } catch (error: unknown) {
      if (context.active) latest.current.onError(messageFromError(error));
      return false;
    }
  }

  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  useEffect(() => {
    const read = () => { void refreshRef.current(); };
    read();
    window.addEventListener("focus", read);
    return () => window.removeEventListener("focus", read);
  }, [context]);

  async function save(sheetId: string) {
    if (context.busy || !context.active) return;
    dismissNotice();
    const savingNoticeContext = noticeContext.current;
    context.busy = true;
    setBusy(true);
    let saved: SaveCustomLayoutResult | null = null;
    const outcome = await input.runner.run(async (port, projection) => {
      saved = await port.saveCustomLayout(sheetId);
      return projection ?? latest.current.projection;
    }, { cancelAfterPendingFailure: true });
    if (!context.active) return;
    context.busy = false;
    setBusy(false);
    if (outcome.status === "failed") latest.current.onError(messageFromError(outcome.error));
    if (outcome.status !== "completed" || !saved) return;
    const result = saved as SaveCustomLayoutResult;
    setRevision((previous) => Math.max(previous, result.catalogRevision));
    setRevealId(result.layoutId);
    if (noticeContext.current === savingNoticeContext) {
      setNotice({ message: result.created ? "Layout salvo em Personalizados." : "Este Layout já está em Personalizados." });
    }
  }

  async function requestDelete(layoutId: CustomLayoutId) {
    if (context.busy || !context.active) return;
    context.busy = true;
    setBusy(true);
    try {
      const result = await context.decisions.run(null, async (decision) => {
        const confirmed = await decision.ask({ kind: "layoutDeletionConfirmation", busy: false },
          (action) => action === "confirmLayoutDeletion" ? true : action === "cancelLayoutDeletion" ? false : undefined);
        if (!confirmed || !await decision.present({ kind: "layoutDeletionConfirmation", busy: true })) return null;
        let observed = 0;
        const outcome = await input.runner.run(async (port, projection) => {
          observed = await port.deleteCustomLayout(layoutId);
          return projection ?? latest.current.projection;
        });
        return { outcome, observed };
      });
      if (!context.active || !result) return;
      if (result.outcome.status === "failed") latest.current.onError(messageFromError(result.outcome.error));
      if (result.outcome.status === "completed") {
        setRevision((previous) => Math.max(previous, result.observed));
        setRevealId((pending) => pending === layoutId ? null : pending);
        setNotice(null);
      }
    } catch (error: unknown) {
      if (context.active) latest.current.onError(messageFromError(error));
    } finally {
      if (context.active) { context.busy = false; setBusy(false); }
    }
  }

  return { revision, busy, notice: notice?.message ?? null, revealId, refresh, save,
    requestDelete: (layoutId: CustomLayoutId) => { void requestDelete(layoutId); },
    acknowledgeReveal: () => setRevealId(null), dismissNotice };
}

function messageFromError(error: unknown) { return error instanceof Error ? error.message : String(error); }

export type LayoutCatalogController = ReturnType<typeof useLayoutCatalog>;
