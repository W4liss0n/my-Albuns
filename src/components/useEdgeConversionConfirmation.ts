import { useLayoutEffect, useMemo } from "react";
import type { ProjectDialogPort } from "../application/projectDialogPort";
import { createProjectDecisions } from "../application/projectDecision";
import { edgeConversionLossDescription, type EdgeConversionLoss } from "../application/edgeConversionReview";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

export function useEdgeConversionConfirmation(
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

  return (loss: EdgeConversionLoss): Promise<boolean> => context.active
    ? context.decisions.run(false, async (decision) => (await decision.ask({
      kind: "edgeConversionConfirmation", message: edgeConversionLossDescription(loss),
    }, (action) => action === "confirmEdgeConversion" ? true
      : action === "cancelEdgeConversion" ? false : undefined)) ?? false,
    { dismissFailure: "reject" })
    : Promise.resolve(false);
}
