import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { ContextMenuSurface } from "./ContextMenuSurface";

test("retains a menu opened on pointer release until a new outside press", () => {
  const onDismiss = vi.fn();
  const view = render(<ContextMenuSurface label="Organizar Frames" position={{ x: 40, y: 40 }} onDismiss={onDismiss}>
    <button role="menuitem">Trazer para frente</button>
  </ContextMenuSurface>);
  const backdrop = view.container.querySelector(".ui-context-menu__dismiss-layer")!;
  // The browser may deliver contextmenu after Pixi's rightclick opened the menu.
  fireEvent.contextMenu(backdrop, { button: 2 });
  expect(onDismiss).not.toHaveBeenCalled();
  expect(screen.getByRole("menu", { name: "Organizar Frames" })).toBeInTheDocument();
  fireEvent.pointerDown(backdrop, { button: 2 });
  fireEvent.contextMenu(backdrop, { button: 2 });
  expect(onDismiss).toHaveBeenCalledOnce();
});
