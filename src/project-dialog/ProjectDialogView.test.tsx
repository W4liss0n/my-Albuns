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
        details: [
          { label: "Lâmina", value: "700 mm × 350 mm" },
          { label: "DPI", value: "240" },
        ],
        kind: "albumInformationConfirmation",
      }}
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "Aplicar alterações no Álbum?",
  });
  expect(
    dialog.querySelector(".album-information-change-list"),
  ).toBeInTheDocument();
  expect(within(dialog).getByText("Lâmina")).toHaveClass(
    "album-information-change__label",
  );
  expect(within(dialog).getByText("700 mm × 350 mm")).toHaveClass(
    "album-information-change__value",
  );
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

test("keeps the Project close confirmation body stable while resolving", () => {
  const { rerender } = render(
    <ProjectDialogView
      onAction={vi.fn()}
      state={{ busy: false, kind: "projectCloseConfirmation" }}
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "Salvar alterações antes de fechar?",
  });
  expect(within(dialog).queryByText("Concluindo…")).not.toBeInTheDocument();
  expect(
    dialog.querySelector(".ui-standard-message__extra"),
  ).not.toBeInTheDocument();

  rerender(
    <ProjectDialogView
      onAction={vi.fn()}
      state={{ busy: true, kind: "projectCloseConfirmation" }}
    />,
  );

  expect(within(dialog).queryByText("Concluindo…")).not.toBeInTheDocument();
  expect(
    dialog.querySelector(".ui-standard-message__extra"),
  ).not.toBeInTheDocument();
  expect(dialog).toHaveTextContent(
    "O Projeto tem alterações que ainda não foram salvas.",
  );
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

test("projects generic operation failures through the standard message dialog", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();

  render(
    <ProjectDialogView
      onAction={onAction}
      state={{
        kind: "projectOperationFailure",
        message: "O Projeto não pôde ser salvo.",
      }}
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "A operação não foi concluída",
  });
  expect(within(dialog).getByRole("alert")).toHaveTextContent(
    "O Projeto não pôde ser salvo.",
  );
  await user.click(within(dialog).getByRole("button", { name: "Fechar" }));
  expect(onAction).toHaveBeenCalledWith("dismissProjectOperationFailure");
});

test("projects a fatal graphics diagnostic through the owned Project dialog", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();

  render(
    <ProjectDialogView
      onAction={onAction}
      state={{
        kind: "graphicsFailure",
        reason: "O contexto WebGL2 foi perdido.",
      }}
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "O Canvas não pôde ser iniciado",
  });
  expect(within(dialog).getByRole("alert")).toHaveTextContent(
    "O contexto WebGL2 foi perdido.",
  );
  await user.click(
    within(dialog).getByRole("button", { name: "Fechar Projeto" }),
  );
  expect(onAction).toHaveBeenCalledWith("closeProjectAfterGraphicsFailure");
});

test("projects export success through the standard message dialog", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();

  render(
    <ProjectDialogView
      onAction={onAction}
      state={{
        kind: "exportSuccess",
        message: "A prova foi exportada com sucesso.",
      }}
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "Exportação concluída",
  });
  expect(within(dialog).getByRole("status")).toHaveTextContent(
    "A prova foi exportada com sucesso.",
  );
  await user.click(within(dialog).getByRole("button", { name: "Fechar" }));
  expect(onAction).toHaveBeenCalledWith("dismissExport");
});
