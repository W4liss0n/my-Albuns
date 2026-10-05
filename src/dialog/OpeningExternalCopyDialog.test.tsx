import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { OpeningExternalCopyDecision } from "../global/application/globalProjectPort";
import { OpeningExternalCopyDialog } from "./OpeningExternalCopyDialog";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((resolver) => { resolve = resolver; });
  return { promise, resolve };
}

function openingExternalCopy(attemptId = "attempt-7") {
  const resolveOpeningExternalCopy = vi.fn(
    async (_attemptId: string, _decision: OpeningExternalCopyDecision): Promise<void> => undefined,
  );
  const view = render(
    <OpeningExternalCopyDialog
      attemptId={attemptId}
      openedFromLoadingOwner={false}
      resolveOpeningExternalCopy={resolveOpeningExternalCopy}
    />,
  );
  return { resolveOpeningExternalCopy, view };
}

function button(name: string) {
  return screen.getByRole("button", { name });
}

test.each([
  ["Salvar cópia como…", "saveCopyAs"],
  ["Cancelar", "cancel"],
] as const)("%s resolves the opening attempt as %s", async (label, decision) => {
  const { resolveOpeningExternalCopy } = openingExternalCopy();

  await act(async () => { fireEvent.click(button(label)); });

  expect(resolveOpeningExternalCopy).toHaveBeenCalledExactlyOnceWith("attempt-7", decision);
});

test("Escape cancels the opening attempt", async () => {
  const { resolveOpeningExternalCopy } = openingExternalCopy();

  await act(async () => { fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" }); });

  expect(resolveOpeningExternalCopy).toHaveBeenCalledExactlyOnceWith("attempt-7", "cancel");
});

test("sends one decision while the opening attempt is resolving", async () => {
  const { resolveOpeningExternalCopy } = openingExternalCopy();
  const pending = deferred();
  resolveOpeningExternalCopy.mockReturnValueOnce(pending.promise);

  fireEvent.click(button("Salvar cópia como…"));

  for (const label of ["Salvar cópia como…", "Cancelar"]) {
    expect(button(label)).toBeDisabled();
    fireEvent.click(button(label));
  }
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(resolveOpeningExternalCopy).toHaveBeenCalledExactlyOnceWith("attempt-7", "saveCopyAs");

  // The Host closes this window on success; the dialog stays resolving.
  await act(async () => pending.resolve());
  expect(button("Salvar cópia como…")).toBeDisabled();
});

test.each([
  ["Salvar cópia como…", "saveCopyAs"],
  ["Cancelar", "cancel"],
] as const)("a rejected %s shows the error and lets the user decide again", async (label, decision) => {
  const { resolveOpeningExternalCopy } = openingExternalCopy();
  resolveOpeningExternalCopy.mockRejectedValueOnce(new Error("Destino indisponível."));

  await act(async () => { fireEvent.click(button(label)); });

  expect(screen.getByRole("dialog")).toHaveTextContent("Destino indisponível.");
  expect(button("Salvar cópia como…")).toBeEnabled();
  expect(button("Cancelar")).toBeEnabled();

  await act(async () => { fireEvent.click(button(label)); });

  expect(resolveOpeningExternalCopy.mock.calls).toEqual([
    ["attempt-7", decision],
    ["attempt-7", decision],
  ]);
  // The retry clears the previous failure.
  expect(screen.getByRole("dialog")).not.toHaveTextContent("Destino indisponível.");
});

test("a rejection without a message shows the external-copy fallback", async () => {
  const { resolveOpeningExternalCopy } = openingExternalCopy();
  resolveOpeningExternalCopy.mockRejectedValueOnce("failed");

  await act(async () => { fireEvent.click(button("Salvar cópia como…")); });

  expect(screen.getByRole("dialog")).toHaveTextContent(
    "Não foi possível concluir a decisão sobre a Cópia externa.",
  );
});

test("an opening attempt without identity explains itself and sends no decision", async () => {
  const { resolveOpeningExternalCopy } = openingExternalCopy("");

  expect(screen.getByRole("dialog")).toHaveTextContent(
    "A tentativa de abertura não está mais disponível.",
  );
  await act(async () => { fireEvent.click(button("Salvar cópia como…")); });
  await act(async () => { fireEvent.click(button("Cancelar")); });

  expect(resolveOpeningExternalCopy).not.toHaveBeenCalled();
});

test.each([true, false])("marks whether the dialog replaced the loading owner (%s)", (openedFromLoadingOwner) => {
  render(
    <OpeningExternalCopyDialog
      attemptId="attempt-7"
      openedFromLoadingOwner={openedFromLoadingOwner}
      resolveOpeningExternalCopy={async () => undefined}
    />,
  );

  expect(screen.getByRole("dialog").closest("[data-opening-owner-transition]"))
    .toHaveAttribute("data-opening-owner-transition", String(openedFromLoadingOwner));
});
