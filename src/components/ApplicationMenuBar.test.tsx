import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import {
  ApplicationMenuBar,
  type ApplicationMenuGroup,
} from "./ApplicationMenuBar";

function menuFixture(onSave = vi.fn()): readonly ApplicationMenuGroup[] {
  return [
    {
      id: "file",
      label: "Arquivo",
      items: [
        {
          availability: "placeholder",
          feature: "open-project-from-project-window",
          id: "open",
          label: "Abrir Projeto…",
          type: "command",
        },
        { id: "file-separator", type: "separator" },
        {
          availability: "implemented",
          id: "save",
          label: "Salvar",
          onSelect: onSave,
          shortcut: "Ctrl+S",
          type: "command",
        },
      ],
    },
    {
      id: "edit",
      label: "Editar",
      items: [
        {
          availability: "implemented",
          disabled: true,
          id: "undo",
          label: "Desfazer",
          onSelect: vi.fn(),
          shortcut: "Ctrl+Z",
          type: "command",
        },
        {
          id: "arrange",
          label: "Organizar",
          type: "submenu",
          items: [
            {
              availability: "placeholder",
              feature: "bring-frames-to-front",
              id: "bring-to-front",
              label: "Trazer para frente",
              type: "command",
            },
          ],
        },
      ],
    },
  ];
}

test("distinguishes implemented commands from explicit placeholders", async () => {
  const user = userEvent.setup();
  const onSave = vi.fn();
  render(<ApplicationMenuBar groups={menuFixture(onSave)} />);

  await user.click(screen.getByRole("menuitem", { name: "Arquivo" }));

  const placeholder = screen.getByRole("menuitem", {
    name: "Abrir Projeto…",
  });
  expect(placeholder).toBeDisabled();
  expect(placeholder).toHaveAttribute(
    "data-placeholder-feature",
    "open-project-from-project-window",
  );
  expect(placeholder).toHaveAttribute(
    "title",
    "Ainda não disponível nesta versão",
  );

  await user.click(screen.getByRole("menuitem", { name: "Salvar" }));
  expect(onSave).toHaveBeenCalledOnce();
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
});

test("switches an open desktop menu on hover and closes it with Escape", async () => {
  const user = userEvent.setup();
  render(<ApplicationMenuBar groups={menuFixture()} />);

  await user.click(screen.getByRole("menuitem", { name: "Arquivo" }));
  fireEvent.pointerEnter(screen.getByRole("menuitem", { name: "Editar" }));

  expect(screen.getByRole("menu", { name: "Editar" })).toBeInTheDocument();
  expect(screen.queryByRole("menu", { name: "Arquivo" })).not.toBeInTheDocument();

  fireEvent.keyDown(document, { key: "Escape" });
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
});

test("closes an open menu when the command bar becomes unavailable", async () => {
  const user = userEvent.setup();
  const view = render(<ApplicationMenuBar groups={menuFixture()} />);

  await user.click(screen.getByRole("menuitem", { name: "Arquivo" }));
  view.rerender(<ApplicationMenuBar disabled groups={menuFixture()} />);

  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  expect(screen.getByRole("menuitem", { name: "Arquivo" })).toBeDisabled();
});

test("supports desktop keyboard navigation and nested menus", async () => {
  render(<ApplicationMenuBar groups={menuFixture()} />);

  const fileMenu = screen.getByRole("menuitem", { name: "Arquivo" });
  fileMenu.focus();
  fireEvent.keyDown(fileMenu, { key: "ArrowDown" });

  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Salvar" })).toHaveFocus(),
  );

  fireEvent.keyDown(screen.getByRole("menuitem", { name: "Salvar" }), {
    key: "ArrowRight",
  });
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Organizar" })).toHaveFocus(),
  );

  fireEvent.keyDown(screen.getByRole("menuitem", { name: "Organizar" }), {
    key: "ArrowRight",
  });
  const submenu = await screen.findByRole("menu", { name: "Organizar" });
  expect(submenu).toHaveFocus();
  expect(
    screen.getByRole("menuitem", { name: "Trazer para frente" }),
  ).toBeDisabled();

  fireEvent.keyDown(submenu, { key: "ArrowLeft" });
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Organizar" })).toHaveFocus(),
  );

  fireEvent.keyDown(screen.getByRole("menuitem", { name: "Organizar" }), {
    key: "Escape",
  });
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Editar" })).toHaveFocus(),
  );
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
});
