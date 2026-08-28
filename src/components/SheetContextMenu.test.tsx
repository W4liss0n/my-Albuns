import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import { SheetContextMenu } from "./SheetContextMenu";

describe("SheetContextMenu", () => {
  test("projects the explicit target and the same structural actions", () => {
    const addBefore = vi.fn();
    const addAfter = vi.fn();
    const deleteSheet = vi.fn();
    const dismiss = vi.fn();
    render(
      <SheetContextMenu
        availability={{
          canAddAfter: true,
          canAddBefore: false,
          canDelete: true,
        }}
        position={{ x: 140, y: 220 }}
        sheetNumber={4}
        onAddAfter={addAfter}
        onAddBefore={addBefore}
        onDelete={deleteSheet}
        onDismiss={dismiss}
      />,
    );

    const menu = screen.getByRole("menu", {
      name: "Ações da Lâmina 04",
    });
    expect(menu).toHaveStyle({ left: "140px", top: "220px" });
    expect(
      screen.getByRole("menuitem", { name: "Adicionar antes" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("menuitem", { name: "Adicionar depois" }),
    ).toBeEnabled();
    expect(
      screen.getByRole("menuitem", { name: "Duplicar Lâmina" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("menuitem", { name: "Converter extremidade" }),
    ).toBeDisabled();

    fireEvent.click(
      screen.getByRole("menuitem", { name: "Adicionar depois" }),
    );
    expect(addAfter).toHaveBeenCalledOnce();
    expect(dismiss).toHaveBeenCalledOnce();
  });

  test("deletes only when the physical minimum permits it", () => {
    const deleteSheet = vi.fn();
    const dismiss = vi.fn();
    const { rerender } = render(
      <SheetContextMenu
        availability={{
          canAddAfter: true,
          canAddBefore: true,
          canDelete: false,
        }}
        position={{ x: 0, y: 0 }}
        sheetNumber={1}
        onAddAfter={vi.fn()}
        onAddBefore={vi.fn()}
        onDelete={deleteSheet}
        onDismiss={dismiss}
      />,
    );
    expect(screen.getByRole("menuitem", { name: "Excluir" })).toBeDisabled();

    rerender(
      <SheetContextMenu
        availability={{
          canAddAfter: true,
          canAddBefore: true,
          canDelete: true,
        }}
        position={{ x: 0, y: 0 }}
        sheetNumber={1}
        onAddAfter={vi.fn()}
        onAddBefore={vi.fn()}
        onDelete={deleteSheet}
        onDismiss={dismiss}
      />,
    );
    fireEvent.click(screen.getByRole("menuitem", { name: "Excluir" }));
    expect(deleteSheet).toHaveBeenCalledOnce();
  });

  test("dismisses with Escape without invoking a structural command", () => {
    const dismiss = vi.fn();
    const deleteSheet = vi.fn();
    render(
      <SheetContextMenu
        availability={{
          canAddAfter: true,
          canAddBefore: true,
          canDelete: true,
        }}
        position={{ x: 4, y: 8 }}
        sheetNumber={2}
        onAddAfter={vi.fn()}
        onAddBefore={vi.fn()}
        onDelete={deleteSheet}
        onDismiss={dismiss}
      />,
    );

    fireEvent.keyDown(document, { key: "Escape" });
    expect(dismiss).toHaveBeenCalledOnce();
    expect(deleteSheet).not.toHaveBeenCalled();
  });
});
