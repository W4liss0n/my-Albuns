import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import { ProjectDialogView } from "./ProjectDialogView";

test("confirms all Album information changes as one action", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();

  render(
    <ProjectDialogView
      onAction={onAction}
      state={{
        busy: false,
        details: ["Lâmina: 700 mm × 350 mm", "DPI: 240"],
        kind: "albumInformationConfirmation",
      }}
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "Aplicar alterações no Álbum?",
  });
  expect(dialog).toHaveTextContent("Lâmina: 700 mm × 350 mm");
  await user.click(within(dialog).getByRole("button", { name: "Aplicar" }));
  await user.click(within(dialog).getByRole("button", { name: "Cancelar" }));
  expect(onAction.mock.calls).toEqual([
    ["confirmAlbumInformation"],
    ["cancelAlbumInformation"],
  ]);
});

test("projects close decisions through the standard confirmation dialog", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();

  render(
    <ProjectDialogView
      onAction={onAction}
      state={{ busy: false, kind: "projectCloseConfirmation" }}
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "Salvar alterações antes de fechar?",
  });
  await user.click(
    within(dialog).getByRole("button", { name: "Salvar e fechar" }),
  );
  await user.click(
    within(dialog).getByRole("button", { name: "Descartar e fechar" }),
  );
  await user.click(within(dialog).getByRole("button", { name: "Cancelar" }));

  expect(onAction.mock.calls).toEqual([
    ["saveAndClose"],
    ["discardAndClose"],
    ["cancelProjectClose"],
  ]);
});

test("projects export progress and cancellation through the standard progress dialog", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();

  render(
    <ProjectDialogView
      onAction={onAction}
      state={{
        cancelRequested: false,
        cancellable: true,
        kind: "exportProgress",
        progress: {
          completed: 2,
          kind: "determinate",
          status: "Compondo a prova",
          total: 5,
        },
      }}
    />,
  );

  expect(screen.getByRole("progressbar")).toHaveAttribute(
    "aria-valuenow",
    "2",
  );
  await user.click(
    screen.getByRole("button", { name: "Cancelar Exportação" }),
  );
  expect(onAction).toHaveBeenCalledWith("cancelExport");
});

test("projects export failure through the standard message dialog", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();

  render(
    <ProjectDialogView
      onAction={onAction}
      state={{
        cancelled: false,
        kind: "exportFailure",
        message: "A mídia original não está disponível.",
        retryDisabled: false,
      }}
    />,
  );

  expect(screen.getByRole("alert")).toHaveTextContent(
    "A mídia original não está disponível.",
  );
  await user.click(screen.getByRole("button", { name: "Tentar novamente" }));
  await user.click(screen.getByRole("button", { name: "Fechar" }));
  expect(onAction.mock.calls).toEqual([
    ["retryExport"],
    ["dismissExport"],
  ]);
});
