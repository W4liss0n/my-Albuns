import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { ProjectRecoveryDialog } from "./ProjectRecoveryDialog";

function recoveryDialog(
  overrides: Partial<Parameters<typeof ProjectRecoveryDialog>[0]> = {},
) {
  const props: Parameters<typeof ProjectRecoveryDialog>[0] = {
    error: null,
    onBack: vi.fn(),
    onDefer: vi.fn(),
    onDiscard: vi.fn(),
    onRecover: vi.fn(),
    onRequestDiscard: vi.fn(),
    state: "available",
    ...overrides,
  };
  return { props, view: render(<ProjectRecoveryDialog {...props} />) };
}

test("renders one accessible external dialog and traps focus on its three decisions", () => {
  const { props } = recoveryDialog();
  const dialog = screen.getByRole("dialog", {
    name: "Recuperar trabalho não salvo?",
  });
  const recover = screen.getByRole("button", {
    name: "Recuperar e abrir",
  });
  const defer = screen.getByRole("button", { name: "Agora não" });

  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(dialog).toHaveAttribute("aria-modal", "true");
  expect(dialog.closest(".project-recovery-dialog-scope")).toBeInTheDocument();
  expect(document.querySelector(".ui-modal-dialog-layer")).not.toBeInTheDocument();
  expect(recover).toHaveFocus();

  fireEvent.keyDown(recover, { key: "Tab" });
  expect(defer).toHaveFocus();
  fireEvent.keyDown(defer, { key: "Tab", shiftKey: true });
  expect(recover).toHaveFocus();

  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(props.onDefer).toHaveBeenCalledOnce();
  expect(props.onRecover).not.toHaveBeenCalled();
});

const callbacks = ["onBack", "onDefer", "onDiscard", "onRecover", "onRequestDiscard"] as const;

test.each([
  ["Recuperar e abrir", "onRecover"],
  ["Abrir última versão salva", "onRequestDiscard"],
  ["Agora não", "onDefer"],
] as const)("%s reports only %s", (label, callback) => {
  const { props } = recoveryDialog();

  fireEvent.click(screen.getByRole("button", { name: label }));

  for (const name of callbacks) {
    expect(props[name]).toHaveBeenCalledTimes(name === callback ? 1 : 0);
  }
});

test.each([
  ["Descartar alterações e abrir", "onDiscard"],
  ["Voltar", "onBack"],
] as const)("%s in the discard confirmation reports only %s", (label, callback) => {
  const { props } = recoveryDialog({ state: "confirmDiscard" });

  fireEvent.click(screen.getByRole("button", { name: label }));

  for (const name of callbacks) {
    expect(props[name]).toHaveBeenCalledTimes(name === callback ? 1 : 0);
  }
});

test.each(["available", "confirmDiscard", "resolving"] as const)(
  "shows the error inside the %s dialog and nothing when there is none",
  (state) => {
    const { props, view } = recoveryDialog({ state, error: "Projeto indisponível." });
    const notice = screen.getByText("Projeto indisponível.").closest(".ui-inline-notice");

    expect(notice).toHaveClass("ui-inline-notice--error");
    expect(screen.getByRole("dialog")).toContainElement(notice as HTMLElement);

    view.rerender(<ProjectRecoveryDialog {...props} error={null} />);
    expect(document.querySelector(".ui-inline-notice")).not.toBeInTheDocument();
  },
);

test("keeps discard confirmation in the same external owner and cancels it with Escape", () => {
  const { props } = recoveryDialog({ state: "confirmDiscard" });
  const dialog = screen.getByRole("dialog", {
    name: "Descartar o trabalho recuperável?",
  });

  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(
    screen.getByRole("button", { name: "Descartar alterações e abrir" }),
  ).toHaveFocus();
  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(props.onBack).toHaveBeenCalledOnce();
  expect(props.onDiscard).not.toHaveBeenCalled();
});


test("does not duplicate a decision while its resolution is in flight", () => {
  const { props } = recoveryDialog({ state: "resolving" });
  const dialog = screen.getByRole("dialog", {
    name: "Recuperar trabalho não salvo?",
  });

  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(screen.getByRole("button", { name: "Agora não" })).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Abrir última versão salva" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Recuperar e abrir" }),
  ).toBeDisabled();
  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(props.onDefer).not.toHaveBeenCalled();
});

test("names the Project when the opening window lists several Projects", () => {
  const { props, view } = recoveryDialog({ projectName: "SARAH XAVIER" });
  expect(screen.getByRole("dialog", { name: "Recuperar trabalho não salvo?" }))
    .toHaveTextContent("Há alterações não salvas no projeto SARAH XAVIER. Deseja recuperá-las?");

  view.rerender(<ProjectRecoveryDialog {...props} state="confirmDiscard" />);
  expect(screen.getByRole("dialog", { name: "Descartar o trabalho recuperável?" }))
    .toHaveTextContent(
      "As alterações não salvas do projeto SARAH XAVIER serão descartadas definitivamente. O projeto abrirá na última versão salva.",
    );

  view.rerender(<ProjectRecoveryDialog {...props} projectName={null} />);
  expect(screen.getByRole("dialog", { name: "Recuperar trabalho não salvo?" }))
    .toHaveTextContent("Há alterações não salvas deste projeto. Deseja recuperá-las?");
});
