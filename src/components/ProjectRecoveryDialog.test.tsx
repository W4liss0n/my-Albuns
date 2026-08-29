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

test("owns one accessible modal and traps focus on its three decisions", () => {
  const { props } = recoveryDialog();
  const dialog = screen.getByRole("dialog", {
    name: "Recuperar trabalho não salvo?",
  });
  const recover = screen.getByRole("button", {
    name: "Reabrir e recuperar",
  });
  const defer = screen.getByRole("button", { name: "Agora não" });

  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(dialog).toHaveAttribute("aria-modal", "true");
  expect(dialog.closest(".ui-modal-dialog-layer")).toHaveAttribute(
    "data-modal-owner",
    "project",
  );
  expect(recover).toHaveFocus();

  fireEvent.keyDown(recover, { key: "Tab" });
  expect(defer).toHaveFocus();
  fireEvent.keyDown(defer, { key: "Tab", shiftKey: true });
  expect(recover).toHaveFocus();

  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(props.onDefer).toHaveBeenCalledOnce();
  expect(props.onRecover).not.toHaveBeenCalled();
});

test("keeps discard confirmation in the same modal owner and cancels it with Escape", () => {
  const { props } = recoveryDialog({ state: "confirmDiscard" });
  const dialog = screen.getByRole("dialog", {
    name: "Descartar o trabalho recuperável?",
  });

  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(
    screen.getByRole("button", { name: "Descartar recuperação e abrir" }),
  ).toHaveFocus();
  fireEvent.pointerDown(document.querySelector(".ui-modal-dialog-layer")!);
  expect(props.onBack).not.toHaveBeenCalled();
  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(props.onBack).toHaveBeenCalledOnce();
  expect(props.onDiscard).not.toHaveBeenCalled();
});

test("restores focus after the modal reaches a terminal", () => {
  const trigger = document.createElement("button");
  document.body.append(trigger);
  trigger.focus();
  const { view } = recoveryDialog();

  expect(trigger).not.toHaveFocus();
  view.unmount();
  expect(trigger).toHaveFocus();
  trigger.remove();
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
    screen.getByRole("button", { name: "Reabrir e recuperar" }),
  ).toBeDisabled();
  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(props.onDefer).not.toHaveBeenCalled();
});
