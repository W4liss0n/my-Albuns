import type { ProjectDialogAction, ProjectDialogPort, ProjectDialogSession, ProjectDialogState } from "./projectDialogPort";

export interface ProjectDecision {
  readonly current: boolean;
  ask<T>(state: ProjectDialogState, accept: (action: ProjectDialogAction) => T | undefined): Promise<T | undefined>;
  present(state: ProjectDialogState): Promise<boolean>;
}

/** Logical decisions share lifetime and settlement, not the port's native queue. */
export function createProjectDecisions(port: ProjectDialogPort) {
  let current: { cancel(): void } | null = null;

  return {
    get busy() { return current !== null; },
    cancel() { current?.cancel(); },
    run<T>(cancelled: T, operation: (decision: ProjectDecision) => Promise<T>,
      options: { dismissFailure?: "ignore" | "reject" } = {}): Promise<T> {
      if (current) return Promise.resolve(cancelled);
      return new Promise<T>((resolve, reject) => {
        let session: ProjectDialogSession | null = null;
        let closing: Promise<void> | null = null;
        let onAction: ((action: ProjectDialogAction) => void) | null = null;
        let cancelQuestion: (() => void) | null = null;
        const dismiss = () => {
          onAction = null;
          if (!closing) {
            try { closing = session?.dismiss() ?? Promise.resolve(); }
            catch (error: unknown) { closing = Promise.reject(error); }
          }
          return closing;
        };
        const execution = { cancel() {
          current = null;
          cancelQuestion?.();
          resolve(cancelled);
          void dismiss().catch(() => undefined);
        } };
        current = execution;
        const decision: ProjectDecision = {
          get current() { return current === execution && !closing; },
          async present(state) {
            if (!decision.current) return false;
            session ??= port.acquire((action) => onAction?.(action));
            await session.present(state);
            return decision.current;
          },
          ask(state, accept) {
            if (!decision.current) return Promise.resolve(undefined);
            return new Promise((answer, fail) => {
              cancelQuestion = () => answer(undefined);
              onAction = (action) => {
                if (!decision.current) return;
                const value = accept(action);
                if (value === undefined) return;
                onAction = null;
                cancelQuestion = null;
                answer(value);
              };
              void decision.present(state).catch(fail);
            });
          },
        };
        void (async () => {
          try {
            const result = await operation(decision);
            await dismiss().catch((error: unknown) => {
              if (options.dismissFailure === "reject") throw error;
            });
            if (current !== execution) return;
            current = null;
            resolve(result);
          } catch (error: unknown) {
            await dismiss().catch(() => undefined);
            if (current !== execution) return;
            current = null;
            reject(error);
          }
        })();
      });
    },
  };
}
