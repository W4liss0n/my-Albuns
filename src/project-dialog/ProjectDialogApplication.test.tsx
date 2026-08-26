import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import type { ProjectDialogPresentation } from "../application/projectDialogPort";
import type { ProjectDialogClient } from "./application/projectDialogClient";
import { ProjectDialogApplication } from "./ProjectDialogApplication";

class ResizeObserverMock {
  disconnect = vi.fn();
  observe = vi.fn();
  unobserve = vi.fn();
}

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", ResizeObserverMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

test("reused dialog actions and window Close follow the latest owner atomically", async () => {
  const user = userEvent.setup();
  const submit = vi.fn(async () => undefined);
  const unlisten = vi.fn();
  let emit!: (presentation: ProjectDialogPresentation) => void;
  const client: ProjectDialogClient = {
    onPresentation: vi.fn(async (listener) => {
      emit = listener;
      return unlisten;
    }),
    submit,
  };
  const windowControls = {
    close: vi.fn(),
    fitContent: vi.fn(),
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
  };
  render(
    <ProjectDialogApplication
      client={client}
      initialPresentation={{
        sessionId: "export-1",
        state: {
          kind: "exportSuccess",
          message: "Primeiro owner",
        },
      }}
      mode="owned"
      windowControls={windowControls}
    />,
  );
  await waitFor(() => expect(client.onPresentation).toHaveBeenCalledOnce());

  act(() => {
    emit({
      sessionId: "project-close-2",
      state: { busy: false, kind: "projectCloseConfirmation" },
    });
  });
  await user.click(screen.getByRole("button", { name: "Cancelar" }));
  await user.click(screen.getByRole("button", { name: "Fechar janela" }));

  expect(submit.mock.calls).toEqual([
    ["project-close-2", "cancelProjectClose"],
    ["project-close-2", "cancelProjectClose"],
  ]);
});
