import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { OpeningProgressDialog } from "./OpeningProgressDialog";

test("opening switches from window preparation to measured image preparation", () => {
  const { rerender } = render(<OpeningProgressDialog />);
  const bar = screen.getByRole("progressbar");
  expect(screen.getByRole("status")).toHaveTextContent("Preparando a Janela do projeto");
  expect(bar).not.toHaveAttribute("aria-valuenow");
  expect(screen.queryByText(/%/)).not.toBeInTheDocument();

  rerender(<OpeningProgressDialog images={{ completedFiles: 5, totalFiles: 12 }} />);
  expect(screen.getByRole("progressbar")).toBe(bar);
  expect(screen.getByRole("status")).toHaveTextContent("Preparando imagens");
  expect(bar).toHaveAttribute("aria-valuenow", "5");
  expect(bar).toHaveAttribute("aria-valuemax", "12");
  expect(screen.getByText("42%")).toBeInTheDocument();
  expect(screen.getByText("5 de 12")).toBeInTheDocument();
  expect(screen.queryByRole("button")).not.toBeInTheDocument();

  rerender(<OpeningProgressDialog images={{ completedFiles: 12, totalFiles: 12 }} />);
  expect(screen.getByText("100%")).toBeInTheDocument();
  expect(screen.getByText("12 de 12")).toBeInTheDocument();
});
