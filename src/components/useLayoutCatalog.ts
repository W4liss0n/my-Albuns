import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ProjectDialogPort, ProjectDialogSession } from "../application/projectDialogPort";
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
  const context = useMemo(() => ({ active: false, busy: false, dialog: null as ProjectDialogSession | null }),
    [input.projection.state.projectId, input.runner, input.dialogPort, input.port]);
  const [revision, setRevision] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [revealId, setRevealId] = useState<CustomLayoutId | null>(null);

  useLayoutEffect(() => {
    context.active = true;
    setBusy(false);
    setNotice(null);
    setRevealId(null);
    return () => {
      context.active = false;
      void context.dialog?.dismiss().catch(() => undefined);
      context.dialog = null;
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
    setNotice(result.created ? "Layout salvo em Personalizados." : "Este Layout já está em Personalizados.");
  }

  function requestDelete(layoutId: CustomLayoutId) {
    if (context.busy || !context.active) return;
    context.busy = true;
    setBusy(true);
    let deciding = true;
    const finish = async () => {
      if (context.dialog !== session) return;
      context.dialog = null;
      await session.dismiss().catch(() => undefined);
      if (!context.active) return;
      context.busy = false;
      setBusy(false);
    };
    const fail = async (error: unknown) => {
      await finish();
      if (context.active) latest.current.onError(messageFromError(error));
    };
    const confirm = async () => {
      try {
        await session.present({ kind: "layoutDeletionConfirmation", busy: true });
        if (!context.active || context.dialog !== session) return;
        let observed = 0;
        const outcome = await input.runner.run(async (port, projection) => {
          observed = await port.deleteCustomLayout(layoutId);
          return projection ?? latest.current.projection;
        });
        if (!context.active) return;
        await finish();
        if (outcome.status === "failed") latest.current.onError(messageFromError(outcome.error));
        if (outcome.status === "completed") {
          setRevision((previous) => Math.max(previous, observed));
          setRevealId((pending) => pending === layoutId ? null : pending);
          setNotice(null);
        }
      } catch (error: unknown) { await fail(error); }
    };
    const session = input.dialogPort.acquire((action) => {
      if (!deciding || !context.active || context.dialog !== session) return;
      if (action === "cancelLayoutDeletion") { deciding = false; void finish(); }
      if (action === "confirmLayoutDeletion") { deciding = false; void confirm(); }
    });
    context.dialog = session;
    void session.present({ kind: "layoutDeletionConfirmation", busy: false }).catch(fail);
  }

  return { revision, busy, notice, revealId, refresh, save, requestDelete,
    acknowledgeReveal: () => setRevealId(null), dismissNotice: () => setNotice(null) };
}

function messageFromError(error: unknown) { return error instanceof Error ? error.message : String(error); }

export type LayoutCatalogController = ReturnType<typeof useLayoutCatalog>;
