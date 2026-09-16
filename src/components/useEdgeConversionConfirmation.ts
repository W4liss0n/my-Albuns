import { useLayoutEffect, useMemo } from "react";
import type { ProjectDialogPort, ProjectDialogSession } from "../application/projectDialogPort";
import { edgeConversionLossDescription, type EdgeConversionLoss } from "../application/edgeConversionReview";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

interface PendingDecision {
  session: ProjectDialogSession | null;
  closing: boolean;
  resolve(value: boolean): void;
  reject(error: unknown): void;
}

export function useEdgeConversionConfirmation(
  projectId: string,
  runner: ProjectMutationRunner,
  port: ProjectDialogPort,
) {
  const context = useMemo(() => ({ active: false, pending: null as PendingDecision | null }),
    [projectId, runner, port]);
  useLayoutEffect(() => {
    context.active = true;
    return () => {
      context.active = false;
      const pending = context.pending;
      context.pending = null;
      pending?.resolve(false);
      if (pending && !pending.closing) void pending.session?.dismiss().catch(() => undefined);
    };
  }, [context]);

  return (loss: EdgeConversionLoss): Promise<boolean> => {
    if (!context.active || context.pending) return Promise.resolve(false);
    return new Promise((resolve, reject) => {
      const pending: PendingDecision = { session: null, closing: false, resolve, reject };
      context.pending = pending;
      const fail = (error: unknown) => {
        if (context.pending !== pending) return;
        context.pending = null;
        void pending.session?.dismiss().catch(() => undefined);
        reject(error);
      };
      try {
        pending.session = port.acquire((action) => {
          if (context.pending !== pending || pending.closing ||
              (action !== "confirmEdgeConversion" && action !== "cancelEdgeConversion")) return;
          pending.closing = true;
          void pending.session!.dismiss().then(() => {
            if (context.pending !== pending) return;
            context.pending = null;
            resolve(context.active && action === "confirmEdgeConversion");
          }, fail);
        });
        void pending.session.present({
          kind: "edgeConversionConfirmation",
          message: edgeConversionLossDescription(loss),
        }).catch(fail);
      } catch (error: unknown) { fail(error); }
    });
  };
}
