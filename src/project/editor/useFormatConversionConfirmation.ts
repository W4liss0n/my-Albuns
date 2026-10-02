import { useLayoutEffect, useMemo } from "react";
import type { ProjectDialogPort } from "../../application/projectDialogPort";
import { createProjectDecisions } from "../../application/projectDecision";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

/** Asks once before the first save replaces an old myAlbuns file (ADR 0012). */
export function useFormatConversionConfirmation(
  projectId: string,
  runner: ProjectMutationRunner,
  port: ProjectDialogPort,
) {
  const context = useMemo(() => ({ active: false, decisions: createProjectDecisions(port) }),
    [projectId, runner, port]);
  useLayoutEffect(() => {
    context.active = true;
    return () => { context.active = false; context.decisions.cancel(); };
  }, [context]);

  return (): Promise<boolean> => context.active
    ? context.decisions.run(false, async (decision) => (await decision.ask({
      kind: "formatConversionSaveConfirmation",
    }, (action) => action === "confirmFormatConversionSave" ? true
      : action === "cancelFormatConversionSave" ? false : undefined)) ?? false,
    { dismissFailure: "reject" })
    : Promise.resolve(false);
}
