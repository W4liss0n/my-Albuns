import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import { ContextMenuSurface } from "./ContextMenuSurface";
import { MenuItem, MenuSeparator } from "./MenuItem";

test("wraps enabled context commands without adding lateral navigation", async () => {
  const user = userEvent.setup();
  const select = vi.fn();
  render(<ContextMenuSurface label="Ações" position={{ x: 40, y: 40 }} onDismiss={vi.fn()}>
    <MenuItem disabled label="Indisponível" />
    <MenuItem label="Primeiro" onClick={select} />
    <MenuSeparator />
    <MenuItem disabled label="Também indisponível" />
    <MenuItem label="Último" />
  </ContextMenuSurface>);
  const first = screen.getByRole("menuitem", { name: "Primeiro" });
  const last = screen.getByRole("menuitem", { name: "Último" });
  await waitFor(() => expect(first).toHaveFocus());
  await user.keyboard("{ArrowUp}");
  expect(last).toHaveFocus();
  await user.keyboard("{ArrowDown}{ArrowRight}{End}");
  expect(first).toHaveFocus();
  await user.keyboard("{Enter}");
  expect(select).toHaveBeenCalledOnce();
});

test("retains a menu opened on pointer release until a new outside press", () => {
  const onDismiss = vi.fn();
  const view = render(<ContextMenuSurface label="Organizar quadros" position={{ x: 40, y: 40 }} onDismiss={onDismiss}>
    <button role="menuitem">Trazer para frente</button>
  </ContextMenuSurface>);
  const backdrop = view.container.querySelector(".ui-context-menu__dismiss-layer")!;
  // The browser may deliver contextmenu after Pixi's rightclick opened the menu.
  fireEvent.contextMenu(backdrop, { button: 2 });
  expect(onDismiss).not.toHaveBeenCalled();
  expect(screen.getByRole("menu", { name: "Organizar quadros" })).toBeInTheDocument();
  fireEvent.pointerDown(backdrop, { button: 2 });
  fireEvent.contextMenu(backdrop, { button: 2 });
  expect(onDismiss).toHaveBeenCalledOnce();
});
