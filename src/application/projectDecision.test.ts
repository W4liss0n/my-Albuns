import { expect, test, vi } from "vitest";
import type { ProjectDialogAction, ProjectDialogPort } from "./projectDialogPort";
import { createProjectDecisions } from "./projectDecision";

test("accepts a decision once and holds completion until its dialog is dismissed", async () => {
  let action!: (action: ProjectDialogAction) => void;
  let release!: () => void;
  const dismiss = vi.fn(() => new Promise<void>((resolve) => { release = resolve; }));
  const port: ProjectDialogPort = { acquire: (listener) => {
    action = listener;
    return { present: async () => undefined, dismiss };
  } };
  const decisions = createProjectDecisions(port);
  const commit = vi.fn();
  const result = decisions.run(false, async (decision) => {
    const confirmed = await decision.ask({ kind: "layoutDeletionConfirmation", busy: false },
      (value) => value === "confirmLayoutDeletion" ? true : undefined);
    if (!confirmed) return false;
    commit();
    return true;
  });
  action("confirmLayoutDeletion");
  action("confirmLayoutDeletion");
  await vi.waitFor(() => expect(dismiss).toHaveBeenCalledOnce());
  expect(commit).toHaveBeenCalledOnce();
  expect(decisions.busy).toBe(true);
  expect(await decisions.run(false, async () => true)).toBe(false);
  release();
  expect(await result).toBe(true);
  expect(decisions.busy).toBe(false);
});

test("cancels a pending decision without letting its late work close a replacement", async () => {
  const sessions: { action: (action: ProjectDialogAction) => void; dismiss: ReturnType<typeof vi.fn> }[] = [];
  const decisions = createProjectDecisions({ acquire: (action) => {
    const session = { action, dismiss: vi.fn(async () => undefined) };
    sessions.push(session);
    return { ...session, present: async () => undefined };
  } });
  let release!: () => void;
  const pending = decisions.run(false, async (decision) => {
    await decision.present({ kind: "layoutDeletionConfirmation", busy: true });
    await new Promise<void>((resolve) => { release = resolve; });
    expect(await decision.present({ kind: "layoutDeletionConfirmation", busy: false })).toBe(false);
    return true;
  });
  await vi.waitFor(() => expect(release).toBeTypeOf("function"));
  decisions.cancel();
  expect(await pending).toBe(false);
  const replacement = decisions.run(false, async (decision) => (await decision.ask(
    { kind: "layoutDeletionConfirmation", busy: false }, () => true)) ?? false);
  release();
  await Promise.resolve();
  sessions[0].action("confirmLayoutDeletion");
  expect(sessions[1].dismiss).not.toHaveBeenCalled();
  sessions[1].action("confirmLayoutDeletion");
  expect(await replacement).toBe(true);
  expect(sessions[0].dismiss).toHaveBeenCalledOnce();
});

test("reopens a decision in the same dialog when the owning flow requires review", async () => {
  let action!: (action: ProjectDialogAction) => void;
  const present = vi.fn(async () => undefined);
  const acquire = vi.fn((listener: typeof action) => {
    action = listener;
    return { present, dismiss: async () => undefined };
  });
  const decisions = createProjectDecisions({ acquire });
  const result = decisions.run(false, async (decision) => {
    const state = { kind: "layoutDeletionConfirmation" as const, busy: false };
    if (!await decision.ask(state, () => true)) return false;
    await decision.present({ ...state, busy: true });
    return (await decision.ask(state, () => true)) ?? false;
  });
  action("confirmLayoutDeletion");
  action("confirmLayoutDeletion");
  await vi.waitFor(() => expect(present).toHaveBeenCalledTimes(3));
  expect(decisions.busy).toBe(true);
  action("confirmLayoutDeletion");
  expect(await result).toBe(true);
  expect(acquire).toHaveBeenCalledOnce();
});

test.each(["acquire", "present", "busy"] as const)("settles %s failures and accepts a later request", async (stage) => {
  const error = new Error("dialog unavailable");
  let action!: (action: ProjectDialogAction) => void;
  const present = vi.fn(async () => undefined);
  if (stage === "present") present.mockRejectedValueOnce(error);
  if (stage === "busy") present.mockResolvedValueOnce(undefined).mockRejectedValueOnce(error);
  const dismiss = vi.fn(async () => undefined);
  const port: ProjectDialogPort = { acquire: (listener) => {
    if (stage === "acquire") throw error;
    action = listener;
    return { present, dismiss };
  } };
  const decisions = createProjectDecisions(port);
  const failed = decisions.run(false, async (decision) => {
    if (!await decision.ask({ kind: "layoutDeletionConfirmation", busy: false }, () => true)) return false;
    await decision.present({ kind: "layoutDeletionConfirmation", busy: true });
    return true;
  });
  const assertion = expect(failed).rejects.toBe(error);
  if (stage === "busy") action("confirmLayoutDeletion");
  await assertion;
  expect(decisions.busy).toBe(false);
  expect(await decisions.run(false, async () => true)).toBe(true);
  expect(dismiss).toHaveBeenCalledTimes(stage === "acquire" ? 0 : 1);
});

test.each(["ignore", "reject"] as const)("preserves the flow's %s policy for a dismissal failure", async (dismissFailure) => {
  const error = new Error("cannot dismiss");
  const decisions = createProjectDecisions({ acquire: () => ({
    present: async () => undefined, dismiss: async () => { throw error; },
  }) });
  const result = decisions.run(false, async (decision) => {
    await decision.present({ kind: "layoutDeletionConfirmation", busy: true });
    return true;
  }, { dismissFailure });
  if (dismissFailure === "ignore") expect(await result).toBe(true);
  else await expect(result).rejects.toBe(error);
  expect(decisions.busy).toBe(false);
});
