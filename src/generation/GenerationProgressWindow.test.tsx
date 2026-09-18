import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { ProjectGenerationPort } from "../application/projectGeneration";
import { GenerationProgressWindow } from "./GenerationProgressWindow";

beforeEach(() => vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} unobserve() {} }));
afterEach(() => vi.unstubAllGlobals());

test("shows the shared progress while the initial preparation state is pending", () => {
  const port: Pick<ProjectGenerationPort, "progress" | "onProgress" | "cancel"> = {
    progress: () => new Promise(() => {}), onProgress: async () => () => {}, cancel: async () => {},
  };
  render(<GenerationProgressWindow port={port} />);
  expect(screen.getByRole("progressbar", { name: "Progresso de Gerando projetos" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Cancelar" })).toBeEnabled();
});

test("changes from unknown work to the shared project counter in the same dialog", async () => {
  let publish!: Parameters<ProjectGenerationPort["onProgress"]>[0];
  const port: Pick<ProjectGenerationPort, "progress" | "onProgress" | "cancel"> = {
    progress: async () => ({ completed: 0, total: null }),
    onProgress: async callback => { publish = callback; return () => {}; }, cancel: async () => {},
  };
  render(<GenerationProgressWindow port={port} />);
  const bar = screen.getByRole("progressbar");
  const dialog = screen.getByRole("dialog");
  expect(bar).not.toHaveAttribute("aria-valuenow");
  await act(async () => publish({ completed: 1, total: 4 }));
  expect(screen.getByRole("dialog")).toBe(dialog);
  expect(screen.getByRole("progressbar")).toBe(bar);
  expect(bar).toHaveAttribute("aria-valuenow", "1");
  expect(screen.getByText("25%")).toBeVisible();
  expect(screen.getByText("1 projeto de 4")).toBeVisible();
});
