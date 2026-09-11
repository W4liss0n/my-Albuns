import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import { ProjectDialogView } from "./ProjectDialogView";

test("custom Layout deletion explains its global scope and offers Cancel and Delete", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();
  const view = render(<ProjectDialogView onAction={onAction} state={{ kind: "layoutDeletionConfirmation", busy: false }} />);
  const dialog = screen.getByRole("dialog", { name: "Excluir Layout personalizado?" });
  expect(dialog).toHaveTextContent("O Layout será removido do catálogo em todas as Janelas.");
  await user.click(within(dialog).getByRole("button", { name: "Cancelar" }));
  await user.click(within(dialog).getByRole("button", { name: "Excluir" }));
  expect(onAction.mock.calls).toEqual([["cancelLayoutDeletion"], ["confirmLayoutDeletion"]]);
  view.rerender(<ProjectDialogView onAction={onAction} state={{ kind: "layoutDeletionConfirmation", busy: true }} />);
  expect(screen.getByRole("button", { name: "Cancelar" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Excluindo…" })).toBeDisabled();
});

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
  const closeProjectButton = within(dialog).getByRole("button", {
    name: "Fechar Projeto",
  });
  expect(closeProjectButton).toHaveFocus();
  await user.click(closeProjectButton);
  expect(onAction).toHaveBeenCalledWith("closeProjectAfterGraphicsFailure");
});

test("shows photo import file progress without an unsafe cancel action", () => {
  const onAction = vi.fn();
  render(<ProjectDialogView onAction={onAction} state={{ kind: "imageProcessingProgress",
    progress: { kind: "determinate", completed: 6, total: 12, status: "6 de 12" } }} />);
  const dialog = screen.getByRole("dialog", { name: "Processando Imagens" });
  expect(within(dialog).getByText("6 de 12")).toBeInTheDocument();
  expect(within(dialog).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "6");
  expect(within(dialog).getByRole("progressbar")).toHaveAttribute("aria-valuemax", "12");
  expect(within(dialog).getByText("50%")).toBeInTheDocument();
  expect(within(dialog).queryByRole("button")).not.toBeInTheDocument();
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


test("shows rejected photo files in Problems and closes without a creative action", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();
  render(<ProjectDialogView onAction={onAction} state={{
    kind: "imageProcessingProblems", importedCount: 2,
    problems: [
      { fileName: "quebrada.jpg", reason: "JPEG corrompido" },
      { fileName: "ausente.jpg", reason: "Arquivo indisponível" },
    ],
  }} />);
  const dialog = screen.getByRole("dialog", { name: "Problemas no processamento" });
  expect(within(dialog).getByRole("columnheader", { name: "Arquivo" })).toBeInTheDocument();
  expect(within(dialog).getByRole("columnheader", { name: "Motivo" })).toBeInTheDocument();
  expect(within(dialog).getByRole("row", { name: "quebrada.jpg JPEG corrompido" })).toBeInTheDocument();
  expect(within(dialog).getByText("2 imagens importadas. Confira os arquivos que não puderam ser processados por completo.")).toBeInTheDocument();
  expect(within(dialog).getByRole("button", { name: "Fechar" })).toHaveFocus();
  await user.keyboard("{Escape}");
  expect(onAction).toHaveBeenCalledExactlyOnceWith("dismissImageProcessingProblems");
});

test("shows a resource interruption once without presenting files as rejected", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();
  render(<ProjectDialogView onAction={onAction} state={{
    kind: "imageProcessingProblems", importedCount: 2, problems: [],
    operationProblem: "Não foi possível continuar o processamento por falta de memória.",
  }} />);
  const dialog = screen.getByRole("dialog", { name: "Importação interrompida" });
  expect(within(dialog).getByText(/2 imagens importadas/)).toBeInTheDocument();
  expect(within(dialog).getAllByText(/Não foi possível continuar o processamento/)).toHaveLength(1);
  expect(within(dialog).queryByRole("table")).not.toBeInTheDocument();
  expect(within(dialog).queryByText(/Confira os arquivos/)).not.toBeInTheDocument();
  await user.click(within(dialog).getByRole("button", { name: "Fechar" }));
  expect(onAction).toHaveBeenCalledExactlyOnceWith("dismissImageProcessingProblems");
});

test("export placeholders list the Project, exact position and Open Project action", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();
  render(<ProjectDialogView onAction={onAction} state={{ kind: "exportProblems", projectName: "Álbum da turma",
    problems: [{ sheetId: "sheet-001", sheetNumber: 1, frameId: "frame-003", frameNumber: 3 }] }} />);
  const dialog = screen.getByRole("dialog", { name: "Problemas na Exportação" });
  expect(within(dialog).getByRole("columnheader", { name: "Projeto" })).toBeInTheDocument();
  expect(within(dialog).getByRole("row", { name: "Álbum da turma Lâmina 01, posição 3: Frame vazio. Abrir Projeto" })).toBeInTheDocument();
  await user.click(within(dialog).getByRole("button", { name: "Abrir Projeto" }));
  expect(onAction).toHaveBeenCalledExactlyOnceWith("openExportProject");
});

test("keeps the operation reason separate from genuine file problems", () => {
  render(<ProjectDialogView onAction={vi.fn()} state={{
    kind: "imageProcessingProblems", importedCount: 2,
    operationProblem: "Não foi possível continuar o processamento por falta de memória.",
    problems: [{ fileName: "quebrada.jpg", reason: "JPEG corrompido" }],
  }} />);
  const dialog = screen.getByRole("dialog", { name: "Importação interrompida" });
  expect(within(dialog).getAllByText(/Não foi possível continuar o processamento/)).toHaveLength(1);
  expect(within(dialog).getByRole("table")).not.toHaveTextContent("memória");
  expect(within(dialog).getAllByRole("row")).toHaveLength(2);
  expect(within(dialog).getByRole("row", { name: "quebrada.jpg JPEG corrompido" })).toBeInTheDocument();
});
