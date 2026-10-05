import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { ProjectRecoveryDecision } from "../application/projectPorts";
import { OpeningRecoveryDialog } from "./OpeningRecoveryDialog";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((resolver) => { resolve = resolver; });
  return { promise, resolve };
}

function openingRecovery(attemptId = "attempt-7") {
  const resolveOpeningRecovery = vi.fn(
    async (_attemptId: string, _decision: ProjectRecoveryDecision): Promise<void> => undefined,
  );
  const view = render(
    <OpeningRecoveryDialog
      attemptId={attemptId}
      openedFromLoadingOwner={false}
      resolveOpeningRecovery={resolveOpeningRecovery}
    />,
  );
  return { resolveOpeningRecovery, view };
}

function button(name: string) {
  return screen.getByRole("button", { name });
}

test.each([
  ["Recuperar e abrir", "reopenAndRecover"],
  ["Agora não", "nowNot"],
] as const)("%s resolves the opening attempt as %s", async (label, decision) => {
  const { resolveOpeningRecovery } = openingRecovery();

  await act(async () => { fireEvent.click(button(label)); });

  expect(resolveOpeningRecovery).toHaveBeenCalledExactlyOnceWith("attempt-7", decision);
});

test("discarding recoverable work needs its own confirmation before the decision is sent", async () => {
  const { resolveOpeningRecovery } = openingRecovery();

  fireEvent.click(button("Abrir última versão salva"));

  expect(resolveOpeningRecovery).not.toHaveBeenCalled();
  expect(
    screen.getByRole("dialog", { name: "Descartar o trabalho recuperável?" }),
  ).toBeInTheDocument();

  await act(async () => { fireEvent.click(button("Descartar alterações e abrir")); });

  expect(resolveOpeningRecovery).toHaveBeenCalledExactlyOnceWith(
    "attempt-7",
    "discardCheckpointAndOpenLastSaved",
  );
});

test("Voltar leaves the discard confirmation without deciding anything", () => {
  const { resolveOpeningRecovery } = openingRecovery();
  fireEvent.click(button("Abrir última versão salva"));

  fireEvent.click(button("Voltar"));

  expect(resolveOpeningRecovery).not.toHaveBeenCalled();
  expect(
    screen.getByRole("dialog", { name: "Recuperar trabalho não salvo?" }),
  ).toBeInTheDocument();
  expect(button("Recuperar e abrir")).toBeEnabled();
});

test("sends one decision while the opening attempt is resolving", async () => {
  const { resolveOpeningRecovery } = openingRecovery();
  const pending = deferred();
  resolveOpeningRecovery.mockReturnValueOnce(pending.promise);

  fireEvent.click(button("Recuperar e abrir"));

  for (const label of ["Recuperar e abrir", "Agora não", "Abrir última versão salva"]) {
    expect(button(label)).toBeDisabled();
    fireEvent.click(button(label));
  }
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(resolveOpeningRecovery).toHaveBeenCalledExactlyOnceWith("attempt-7", "reopenAndRecover");
  expect(screen.getAllByRole("dialog")).toHaveLength(1);

  // The Host closes this window on success; the dialog stays resolving.
  await act(async () => pending.resolve());
  expect(button("Recuperar e abrir")).toBeDisabled();
});

test.each([
  ["Recuperar e abrir", "reopenAndRecover"],
  ["Agora não", "nowNot"],
] as const)("a rejected %s shows the error and lets the user decide again", async (label, decision) => {
  const { resolveOpeningRecovery } = openingRecovery();
  resolveOpeningRecovery.mockRejectedValueOnce(new Error("Projeto indisponível."));

  await act(async () => { fireEvent.click(button(label)); });

  expect(
    screen.getByRole("dialog", { name: "Recuperar trabalho não salvo?" }),
  ).toHaveTextContent("Projeto indisponível.");
  for (const name of ["Recuperar e abrir", "Agora não", "Abrir última versão salva"]) {
    expect(button(name)).toBeEnabled();
  }

  await act(async () => { fireEvent.click(button(label)); });

  expect(resolveOpeningRecovery.mock.calls).toEqual([
    ["attempt-7", decision],
    ["attempt-7", decision],
  ]);
  // The retry clears the previous failure.
  expect(screen.getByRole("dialog")).not.toHaveTextContent("Projeto indisponível.");
});

test("a rejected discard returns to its confirmation with the error, and Voltar clears it", async () => {
  const { resolveOpeningRecovery } = openingRecovery();
  resolveOpeningRecovery.mockRejectedValueOnce(new Error("Projeto indisponível."));
  fireEvent.click(button("Abrir última versão salva"));

  await act(async () => { fireEvent.click(button("Descartar alterações e abrir")); });

  expect(
    screen.getByRole("dialog", { name: "Descartar o trabalho recuperável?" }),
  ).toHaveTextContent("Projeto indisponível.");
  expect(button("Descartar alterações e abrir")).toBeEnabled();

  fireEvent.click(button("Voltar"));

  expect(
    screen.getByRole("dialog", { name: "Recuperar trabalho não salvo?" }),
  ).not.toHaveTextContent("Projeto indisponível.");
  expect(resolveOpeningRecovery).toHaveBeenCalledOnce();
});

test("a rejection without a message shows the Recovery fallback", async () => {
  const { resolveOpeningRecovery } = openingRecovery();
  resolveOpeningRecovery.mockRejectedValueOnce({ code: "unknown" });

  await act(async () => { fireEvent.click(button("Recuperar e abrir")); });

  expect(screen.getByRole("dialog")).toHaveTextContent(
    "Não foi possível concluir a escolha de Recuperação.",
  );
});

test("an opening attempt without identity explains itself and sends no decision", async () => {
  const { resolveOpeningRecovery } = openingRecovery("");

  expect(screen.getByRole("dialog")).toHaveTextContent(
    "A tentativa de abertura não está mais disponível.",
  );
  await act(async () => { fireEvent.click(button("Recuperar e abrir")); });
  await act(async () => { fireEvent.click(button("Agora não")); });
  fireEvent.click(button("Abrir última versão salva"));
  await act(async () => { fireEvent.click(button("Descartar alterações e abrir")); });

  expect(resolveOpeningRecovery).not.toHaveBeenCalled();
});

test.each([true, false])("marks whether the dialog replaced the loading owner (%s)", (openedFromLoadingOwner) => {
  render(
    <OpeningRecoveryDialog
      attemptId="attempt-7"
      openedFromLoadingOwner={openedFromLoadingOwner}
      resolveOpeningRecovery={async () => undefined}
    />,
  );

  expect(screen.getByRole("dialog").closest("[data-opening-owner-transition]"))
    .toHaveAttribute("data-opening-owner-transition", String(openedFromLoadingOwner));
});
