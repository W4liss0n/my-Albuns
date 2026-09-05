import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { ExternalCopyDecisionDialog } from "./ExternalCopyDecisionDialog";

function externalCopyDialog(
  overrides: Partial<Parameters<typeof ExternalCopyDecisionDialog>[0]> = {},
) {
  const props: Parameters<typeof ExternalCopyDecisionDialog>[0] = {
    error: null,
    onCancel: vi.fn(),
    onSaveCopyAs: vi.fn(),
    resolving: false,
    ...overrides,
  };
  return { props, view: render(<ExternalCopyDecisionDialog {...props} />) };
}

test("offers only the two external-copy terminals in one accessible external dialog", () => {
  const { props } = externalCopyDialog();
  const dialog = screen.getByRole("dialog", {
    name: "Cópia externa somente leitura",
  });
  const saveCopyAs = screen.getByRole("button", {
    name: "Salvar cópia como…",
  });
  const cancel = screen.getByRole("button", { name: "Cancelar" });

  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(dialog).toHaveAttribute("aria-modal", "true");
  expect(document.querySelector(".ui-modal-dialog-layer")).not.toBeInTheDocument();
  expect(saveCopyAs).toHaveFocus();

  fireEvent.click(saveCopyAs);
  expect(props.onSaveCopyAs).toHaveBeenCalledOnce();
  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(props.onCancel).toHaveBeenCalledOnce();

  fireEvent.keyDown(saveCopyAs, { key: "Tab" });
  expect(cancel).toHaveFocus();
  fireEvent.keyDown(cancel, { key: "Tab", shiftKey: true });
  expect(saveCopyAs).toHaveFocus();
});

test("prevents duplicate decisions while the same Host attempt is resolving", () => {
  const { props } = externalCopyDialog({ resolving: true });
  const dialog = screen.getByRole("dialog", {
    name: "Cópia externa somente leitura",
  });

  expect(screen.getByRole("button", { name: "Cancelar" })).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Salvar cópia como…" }),
  ).toBeDisabled();
  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(props.onCancel).not.toHaveBeenCalled();
  expect(props.onSaveCopyAs).not.toHaveBeenCalled();
});
