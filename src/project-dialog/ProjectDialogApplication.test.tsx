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
        windowWidth: 440,
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
  expect(windowControls.fitContent).toHaveBeenLastCalledWith(expect.any(Function), 440);

  act(() => {
    emit({
      sessionId: "project-close-2",
      windowWidth: 520,
      state: { busy: false, kind: "projectCloseConfirmation" },
    });
  });
  expect(windowControls.fitContent).toHaveBeenLastCalledWith(expect.any(Function), 520);
  expect(document.querySelector(".ui-owned-window-shell")).toHaveStyle({ width: "520px" });
  await user.click(screen.getByRole("button", { name: "Cancelar" }));
  await user.click(screen.getByRole("button", { name: "Fechar janela" }));

  expect(submit.mock.calls).toEqual([
    ["project-close-2", "cancelProjectClose"],
    ["project-close-2", "cancelProjectClose"],
  ]);
});

test("titles every export step with Exportar and never shows the brand in a dialog", () => {
  const windowControls = { close: vi.fn(), fitContent: vi.fn(), minimize: vi.fn(), toggleMaximize: vi.fn() };
  const { rerender } = render(
    <ProjectDialogApplication
      mode="preview"
      state={{
        kind: "exportConfiguration", busy: false, message: "",
        sheets: [{ sheetId: "opening", number: 1, pageCount: 1 }],
        options: { scope: "album", sheetIds: ["opening"], mode: "sheet", format: { kind: "png" }, destination: "C:/álbuns", conflictPolicy: "ask" },
      }}
      windowControls={windowControls}
    />,
  );
  const header = screen.getByRole("banner", { name: "Barra da janela" });
  expect(header).toHaveTextContent("Exportar");
  expect(header.querySelector(".ui-brand")).toBeNull();

  rerender(
    <ProjectDialogApplication
      mode="preview"
      state={{ kind: "exportSuccess", message: "A exportação foi concluída com sucesso." }}
      windowControls={windowControls}
    />,
  );
  expect(screen.getByRole("banner", { name: "Barra da janela" })).toHaveTextContent("Exportar");
  expect(screen.getByRole("banner", { name: "Barra da janela" }).querySelector(".ui-brand")).toBeNull();

  rerender(
    <ProjectDialogApplication
      mode="preview"
      state={{ busy: false, kind: "projectCloseConfirmation" }}
      windowControls={windowControls}
    />,
  );
  const closeHeader = screen.getByRole("banner", { name: "Barra da janela" });
  expect(closeHeader.querySelector(".ui-brand")).toBeNull();
  expect(closeHeader).not.toHaveTextContent("Exportar");
});
