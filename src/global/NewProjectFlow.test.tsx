import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import type { ProjectLaunchOutcome } from "./application/globalProjectPort";
import { NewProjectFlow } from "./NewProjectFlow";

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

test("presents exactly the two neutral creation steps and preserves them while navigating", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
    />,
  );

  expect(
    screen.getByRole("heading", { name: "Dimensões" }),
  ).toBeInTheDocument();
  expect(screen.getByText("mm")).toBeInTheDocument();
  expect(
    screen.getByText("600 × 300 mm (60 × 30 cm)"),
  ).toBeInTheDocument();
  expect(screen.getByText("300 DPI")).toBeInTheDocument();
  expect(screen.getByText("2 Lâminas duplas")).toBeInTheDocument();
  expect(screen.getAllByText("3 mm")).toHaveLength(2);
  expect(screen.queryByLabelText(/nome/i)).not.toBeInTheDocument();
  expect(screen.queryByLabelText(/localização/i)).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Próximo" }));

  expect(
    screen.getByRole("heading", { name: "Personalização" }),
  ).toBeInTheDocument();
  expect(screen.getByText("Background branco")).toBeInTheDocument();
  expect(screen.getByText("Sem Overlay")).toBeInTheDocument();
  expect(screen.getByText("Sem borda")).toBeInTheDocument();
  expect(screen.getByText("Sem Frames ou mídias")).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Voltar" }));
  expect(
    screen.getByText("600 × 300 mm (60 × 30 cm)"),
  ).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Próximo" }));
  expect(screen.getByText("Background branco")).toBeInTheDocument();
});

test("cancels before the native boundary without starting creation", async () => {
  const user = userEvent.setup();
  const onCancel = vi.fn();
  const onCreate = vi.fn(async () => ({ status: "opened" as const }));

  render(<NewProjectFlow onCancel={onCancel} onCreate={onCreate} />);
  await user.click(screen.getByRole("button", { name: "Cancelar" }));

  expect(onCancel).toHaveBeenCalledOnce();
  expect(onCreate).not.toHaveBeenCalled();
});

test("asks for the native destination only on Create and returns to Personalização after cancellation", async () => {
  const user = userEvent.setup();
  const creation = deferred<ProjectLaunchOutcome>();
  const onCreate = vi.fn(() => creation.promise);

  render(<NewProjectFlow onCancel={vi.fn()} onCreate={onCreate} />);
  await user.click(screen.getByRole("button", { name: "Próximo" }));

  expect(onCreate).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "Criar" }));

  expect(onCreate).toHaveBeenCalledOnce();
  expect(onCreate).toHaveBeenCalledWith("neutralV1");
  expect(
    screen.getByRole("button", { name: "Criando Projeto…" }),
  ).toBeDisabled();

  await act(async () => {
    creation.resolve({ status: "cancelled" });
  });

  expect(
    screen.getByRole("heading", { name: "Personalização" }),
  ).toBeInTheDocument();
  expect(screen.getByText("Background branco")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Criar" })).toBeEnabled();
});

test("keeps the completed draft available after a structured creation failure", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn(async () => ({
    status: "failed" as const,
    error: {
      code: "destination_conflict",
      message: "Outro objeto passou a ocupar este destino.",
      action: "Escolha outro nome e tente novamente.",
    },
  }));

  render(<NewProjectFlow onCancel={vi.fn()} onCreate={onCreate} />);
  await user.click(screen.getByRole("button", { name: "Próximo" }));
  await user.click(screen.getByRole("button", { name: "Criar" }));

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Outro objeto passou a ocupar este destino.",
  );
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Escolha outro nome e tente novamente.",
  );
  expect(screen.getByText("Sem Overlay")).toBeInTheDocument();
});
