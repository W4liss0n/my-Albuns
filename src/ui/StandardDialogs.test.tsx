import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import { ConfirmationDialog } from "./ConfirmationDialog";
import { MessageDialog } from "./MessageDialog";
import { ProgressDialog } from "./ProgressDialog";

test("presents indeterminate progress without inventing a percentage", () => {
  render(
    <ProgressDialog
      progress={{
        kind: "indeterminate",
        note: "sem estimativa de tempo",
        status: "Preparando a Janela do Projeto…",
      }}
      title="Abrindo Projeto"
    />,
  );

  const dialog = screen.getByRole("dialog", { name: "Abrindo Projeto" });
  const progressbar = within(dialog).getByRole("progressbar", {
    name: "Progresso de Abrindo Projeto",
  });

  expect(within(dialog).getByRole("status")).toHaveTextContent(
    "Preparando a Janela do Projeto",
  );
  expect(progressbar).not.toHaveAttribute("aria-valuenow");
  expect(within(dialog).queryByRole("button")).not.toBeInTheDocument();
});

test("presents determined and batch progress through the same interface", () => {
  const { rerender } = render(
    <ProgressDialog
      progress={{
        completed: 14,
        kind: "determinate",
        remaining: "cerca de 4 min restantes",
        status: "Lâmina 14 de 40 · Formatura Medicina 2026",
        total: 40,
      }}
      title="Exportando PDF para gráfica"
    />,
  );

  expect(screen.getByRole("progressbar")).toHaveAttribute(
    "aria-valuenow",
    "14",
  );
  expect(screen.getByText("35%")).toBeInTheDocument();
  expect(screen.queryByText("14/40")).not.toBeInTheDocument();

  rerender(
    <ProgressDialog
      progress={{
        completed: 1,
        currentItem: "15 anos Beatriz",
        currentItemStatus: "lâmina 7 de 18",
        kind: "batch",
        summary: "1 concluído · 1 na fila",
        total: 3,
      }}
      title="Exportando 3 álbuns"
    />,
  );

  expect(screen.getByText("15 anos Beatriz")).toBeInTheDocument();
  expect(screen.getByText("Álbum 2 de 3")).toBeInTheDocument();
  expect(screen.getByRole("progressbar")).toHaveAttribute(
    "aria-valuemax",
    "3",
  );
});

test("keeps confirmation actions in their standard semantic positions", async () => {
  const user = userEvent.setup();
  const onConfirm = vi.fn();
  const onDiscard = vi.fn();

  render(
    <ConfirmationDialog
      cancelAction={{ label: "Cancelar", onClick: vi.fn() }}
      confirmAction={{ label: "Excluir lâmina", onClick: onConfirm }}
      description="Os 5 frames desta lâmina serão removidos."
      leadingAction={{ label: "Descartar", onClick: onDiscard }}
      title="Excluir a lâmina 04?"
      tone="danger"
    />,
  );

  const dialog = screen.getByRole("dialog", { name: "Excluir a lâmina 04?" });
  const confirm = within(dialog).getByRole("button", {
    name: "Excluir lâmina",
  });

  expect(confirm).toHaveClass("ui-action-button--danger");
  await user.click(confirm);
  await user.click(within(dialog).getByRole("button", { name: "Descartar" }));
  expect(onConfirm).toHaveBeenCalledOnce();
  expect(onDiscard).toHaveBeenCalledOnce();
});

test("presents standard error messages as an actionable dialog", () => {
  render(
    <MessageDialog
      description="A imagem não está mais na pasta de origem."
      detail="IMG_4417.CR3 · fotos/medicina/brutos"
      primaryAction={{ label: "Localizar arquivo…", onClick: vi.fn() }}
      secondaryAction={{ label: "Fechar", onClick: vi.fn() }}
      title="Não foi possível exportar o PDF"
      tone="error"
    />,
  );

  const dialog = screen.getByRole("dialog", {
    name: "Não foi possível exportar o PDF",
  });

  expect(within(dialog).getByRole("alert")).toHaveTextContent(
    "A imagem não está mais na pasta de origem.",
  );
  expect(
    within(dialog).getByRole("button", { name: "Localizar arquivo…" }),
  ).toHaveClass("ui-action-button--primary");
});
