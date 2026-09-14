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
  expect(
    within(dialog).queryByText("sem estimativa de tempo", { exact: false }),
  ).not.toBeInTheDocument();
  expect(within(dialog).queryByRole("button")).not.toBeInTheDocument();
});

test("shows percentage and an automatic count below every determinate bar", () => {
  const { rerender } = render(
    <ProgressDialog
      progress={{
        completed: 14,
        kind: "determinate",
        total: 40,
      }}
      title="Processando Imagens"
    />,
  );

  expect(screen.getByRole("progressbar")).toHaveAttribute(
    "aria-valuenow",
    "14",
  );
  expect(screen.getByText("35%")).toBeInTheDocument();
  expect(screen.getByText("14 de 40").closest(".ui-progress-dialog__meta"))
    .toBe(screen.getByText("35%").closest(".ui-progress-dialog__meta"));
  expect(screen.queryByRole("status")).not.toBeInTheDocument();

  rerender(
    <ProgressDialog
      progress={{
        completed: 43,
        countLabel: "7 álbuns de 18",
        kind: "determinate",
        total: 100,
      }}
      title="Exportando"
    />,
  );

  expect(screen.getByText("7 álbuns de 18").closest(".ui-progress-dialog__meta"))
    .toBe(screen.getByText("43%").closest(".ui-progress-dialog__meta"));
  expect(screen.queryByText("43 de 100")).not.toBeInTheDocument();
  expect(screen.getByRole("progressbar")).toHaveAttribute(
    "aria-valuemax",
    "100",
  );
});

test("keeps zero and complete counts in the standard row without inventing units", () => {
  const { rerender } = render(<ProgressDialog title="Processando Imagens"
    progress={{ kind: "determinate", completed: 0, total: 0 }} />);
  expect(screen.getByText("0 de 0")).toBeVisible();
  expect(screen.getByText("0%")).toBeVisible();
  rerender(<ProgressDialog title="Processando Imagens"
    progress={{ kind: "determinate", completed: 12, total: 12 }} />);
  expect(screen.getByText("12 de 12")).toBeVisible();
  expect(screen.getByText("100%")).toBeVisible();
  rerender(<ProgressDialog title="Processando Imagens" reserveProgressMeta
    progress={{ kind: "indeterminate", status: "Aguarde…" }} />);
  expect(screen.queryByText("12 de 12")).not.toBeInTheDocument();
  expect(screen.queryByText(/%/)).not.toBeInTheDocument();
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

  expect(confirm).toHaveClass("ui-confirmation-dialog__danger-action");
  expect(confirm).not.toHaveClass("ui-action-button--danger");
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
