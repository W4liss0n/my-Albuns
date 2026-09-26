import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import { ProjectDialogView } from "./ProjectDialogView";

test("export media recovery offers distinct actions without a Continue step", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();
  const state = { kind: "exportMediaProblems" as const, projectName: "Álbum", busy: false, message: "", problems: [
    { mediaId: "photo-1", fileName: "Foto.jpg", state: "absent" as const },
    { mediaId: "photo-2", fileName: "Rede.png", state: "unavailable" as const },
  ] };
  const view = render(<ProjectDialogView state={state} onAction={onAction} />);
  // A single Project: each entry is titled by its file, with the Project on hover.
  const [absent, unavailable] = screen.getAllByRole("listitem");
  expect(within(absent).getByText("Foto.jpg")).toHaveAttribute("title", "Álbum");
  expect(absent).toHaveTextContent("Arquivo ausente.");
  expect(unavailable).toHaveTextContent("Rede.png");
  expect(unavailable).toHaveTextContent("Arquivo indisponível.");
  expect(screen.queryByText("Álbum")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Continuar exportação" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Localizar imagens…" }));
  await user.click(screen.getByRole("button", { name: "Tentar novamente" }));
  expect(onAction.mock.calls).toEqual([["relinkExportMedia"], ["retryExportMedia"]]);
  view.rerender(<ProjectDialogView state={{ ...state, busy: true }} onAction={onAction} />);
  await user.keyboard("{Escape}");
  expect(screen.getByRole("button", { name: "Fechar" })).toBeDisabled();
  expect(onAction).toHaveBeenCalledTimes(2);
});

test("export conflicts use a generic confirmation with skip, replace and cancel", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();
  render(<ProjectDialogView onAction={onAction} state={{ kind: "exportConflicts", files: ["Album_001.png", "Album_002.png"] }} />);
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
  expect(screen.queryByText("Album_001.png")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Ignorar" }));
  await user.click(screen.getByRole("button", { name: "Substituir" }));
  await user.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(onAction.mock.calls).toEqual([["skipExportConflicts"], ["confirmExportOverwrite"], ["dismissExport"]]);
});

test("custom Layout deletion explains its global scope and offers Cancel and Delete", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();
  const view = render(<ProjectDialogView onAction={onAction} state={{ kind: "layoutDeletionConfirmation", busy: false }} />);
  const dialog = screen.getByRole("dialog", { name: "Excluir layout personalizado?" });
  expect(dialog).toHaveTextContent("Este layout será removido da lista de personalizados.");
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
        consequences: [
          "A primeira lâmina vira página única. O fundo da lâmina 1 será removido.",
          "A proporção das lâminas muda. As fotos mantêm a proporção, e o recorte pode ser ajustado.",
        ],
        kind: "albumInformationConfirmation",
      }}
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "Aplicar alterações no álbum?",
  });
  // Only consequences, one sentence each, then the undo note; no list of values.
  expect(within(dialog).getByText(/O fundo da lâmina 1 será removido\./).tagName).toBe("P");
  expect(within(dialog).getByText(/o recorte pode ser ajustado/)).toBeInTheDocument();
  expect(within(dialog).getByText("Você pode desfazer tudo de uma vez.")).toHaveClass("album-information-undo");
  expect(dialog.querySelector("dl")).toBeNull();
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
    "O projeto tem alterações que ainda não foram salvas.",
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
          status: "2 lâminas de 5",
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
    screen.getByRole("button", { name: "Cancelar exportação" }),
  );
  expect(onAction).toHaveBeenCalledWith("cancelExport");
  expect(screen.getByText("2 lâminas de 5")).toBeVisible();
  expect(screen.getByText("2 lâminas de 5").closest(".ui-progress-dialog__meta"))
    .toBe(screen.getByText("40%").closest(".ui-progress-dialog__meta"));
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
  expect(screen.getByText("Exportando")).toBeInTheDocument();
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
        message: "O projeto não pôde ser salvo.",
      }}
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "A operação não foi concluída",
  });
  expect(within(dialog).getByRole("alert")).toHaveTextContent(
    "O projeto não pôde ser salvo.",
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
    name: "Não foi possível iniciar o editor",
  });
  expect(within(dialog).getByRole("alert")).toHaveTextContent(
    "O contexto WebGL2 foi perdido.",
  );
  const closeProjectButton = within(dialog).getByRole("button", {
    name: "Fechar projeto",
  });
  expect(closeProjectButton).toHaveFocus();
  await user.click(closeProjectButton);
  expect(onAction).toHaveBeenCalledWith("closeProjectAfterGraphicsFailure");
});

test("shows photo import file progress without an unsafe cancel action", () => {
  const onAction = vi.fn();
  render(<ProjectDialogView onAction={onAction} state={{ kind: "imageProcessingProgress",
    progress: { kind: "determinate", completed: 6, total: 12, status: "" } }} />);
  const dialog = screen.getByRole("dialog", { name: "Processando imagens" });
  expect(within(dialog).getByText("6 de 12")).toBeInTheDocument();
  expect(within(dialog).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "6");
  expect(within(dialog).getByRole("progressbar")).toHaveAttribute("aria-valuemax", "12");
  expect(within(dialog).getByText("50%")).toBeInTheDocument();
  expect(within(dialog).getByText("6 de 12").closest(".ui-progress-dialog__meta"))
    .toBe(within(dialog).getByText("50%").closest(".ui-progress-dialog__meta"));
  expect(within(dialog).queryByRole("status")).not.toBeInTheDocument();
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
        message: "A exportação foi concluída com sucesso.",
      }}
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "Exportação concluída",
  });
  expect(within(dialog).getByRole("status")).toHaveTextContent(
    "A exportação foi concluída com sucesso.",
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
  const [first] = within(dialog).getAllByRole("listitem");
  expect(first).toHaveTextContent("quebrada.jpg");
  expect(first).toHaveTextContent("JPEG corrompido");
  expect(within(dialog).queryByRole("table")).not.toBeInTheDocument();
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
  expect(within(dialog).queryByRole("list")).not.toBeInTheDocument();
  expect(within(dialog).queryByText(/Confira os arquivos/)).not.toBeInTheDocument();
  await user.click(within(dialog).getByRole("button", { name: "Fechar" }));
  expect(onAction).toHaveBeenCalledExactlyOnceWith("dismissImageProcessingProblems");
});

test("export placeholders list each exact position with the Project on hover and the Open Project action", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();
  render(<ProjectDialogView onAction={onAction} state={{ kind: "exportProblems", projectName: "Álbum da turma",
    problems: [{ sheetId: "sheet-001", sheetNumber: 1, frameId: "frame-003", frameNumber: 3 }] }} />);
  const dialog = screen.getByRole("dialog", { name: "Problemas na exportação" });
  const [entry] = within(dialog).getAllByRole("listitem");
  expect(within(entry).getByText("Lâmina 01, posição 3")).toHaveAttribute("title", "Álbum da turma");
  expect(entry).toHaveTextContent("Quadro vazio.");
  await user.click(within(dialog).getByRole("button", { name: "Voltar ao álbum" }));
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
  expect(within(dialog).getByRole("list")).not.toHaveTextContent("memória");
  const entries = within(dialog).getAllByRole("listitem");
  expect(entries).toHaveLength(1);
  expect(entries[0]).toHaveTextContent("quebrada.jpg");
  expect(entries[0]).toHaveTextContent("JPEG corrompido");
});

test("edge conversion uses the standard confirmation actions and names the discarded application", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();
  render(<ProjectDialogView onAction={onAction} state={{ kind: "edgeConversionConfirmation",
    message: "A sobreposição personalizada da página direita da lâmina 3 será removida." }} />);
  expect(screen.getByRole("heading", { name: "Converter para página única?" })).toBeInTheDocument();
  expect(screen.getByText(/sobreposição personalizada da página direita/)).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Cancelar" }));
  await user.click(screen.getByRole("button", { name: "Converter" }));
  expect(onAction.mock.calls).toEqual([["cancelEdgeConversion"], ["confirmEdgeConversion"]]);
});
