import { render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";
import type { OpeningProjectProgress } from "../contracts/generated/OpeningProjectProgress";
import {
  OpeningProgressDialog,
  OpeningProjectsDialog,
  openingProgressView,
} from "./OpeningProgressDialog";

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

const row = (
  name: string,
  state: OpeningProjectProgress["state"],
  completedFiles = 0,
  totalFiles = 0,
): OpeningProjectProgress => ({ name, state, completedFiles, totalFiles });

test("several Projects opened together are listed in opening order with their own state", () => {
  render(<OpeningProjectsDialog projects={[
    row("YUELSON RODRIGO", "ready", 40, 40),
    row("SARAH XAVIER", "preparing", 32, 70),
    row("SARAH DA SILVA", "starting"),
    row("CLIENTE ÚNICO", "preparing", 0, 1),
    row("PROJETO ANTIGO", "failed"),
    row("CÓPIA EXTERNA", "cancelled"),
  ]} />);

  expect(screen.getByRole("heading", { name: "Abrindo 6 projetos" })).toBeInTheDocument();
  const list = screen.getByRole("list", { name: "Projetos" });
  const rows = within(list).getAllByRole("listitem");
  expect(rows.map((item) => item.textContent)).toEqual([
    "YUELSON RODRIGOPronto",
    "SARAH XAVIER32 de 70 fotos",
    "SARAH DA SILVAAbrindo…",
    "CLIENTE ÚNICO0 de 1 foto",
    "PROJETO ANTIGONão abriu",
    "CÓPIA EXTERNACancelado",
  ]);
  const bars = within(list).getAllByRole("progressbar");
  expect(bars).toHaveLength(2);
  expect(within(rows[1]).getByRole("progressbar", { name: "Progresso de SARAH XAVIER" }))
    .toHaveAttribute("aria-valuenow", "32");
  expect(bars[0]).toHaveAttribute("aria-valuemax", "70");
  expect(within(rows[0]).getByText("YUELSON RODRIGO")).toHaveAttribute("title", "YUELSON RODRIGO");
  expect(screen.queryByRole("button")).not.toBeInTheDocument();
});

test("one Project keeps the single dialog and several Projects use the list", () => {
  expect(openingProgressView(null, false)).toBeNull();
  expect(openingProgressView({ projects: [] }, false)).toBeNull();
  expect(openingProgressView({ projects: [row("A", "starting")] }, false)).toBeNull();
  expect(openingProgressView({ projects: [row("A", "preparing", 5, 12)] }, false))
    .toEqual({ kind: "single", images: row("A", "preparing", 5, 12) });
  expect(openingProgressView({ projects: [row("A", "ready", 12, 12)] }, false))
    .toEqual({ kind: "single", images: row("A", "ready", 12, 12) });
  expect(openingProgressView({ projects: [row("A", "starting"), row("B", "starting")] }, false))
    .toEqual({ kind: "list", projects: [row("A", "starting"), row("B", "starting")] });
});

test("a decision page stays until its decision is no longer pending", () => {
  const pending = { projects: [row("A", "deciding"), row("B", "preparing", 3, 9)] };
  expect(openingProgressView(pending, true)).toBeNull();
  const resolved = { projects: [row("A", "starting"), row("B", "preparing", 3, 9)] };
  expect(openingProgressView(resolved, true)).toEqual({ kind: "list", projects: resolved.projects });
  expect(openingProgressView({ projects: [row("A", "starting")] }, true)).toBeNull();
});

test("finished Projects are announced politely, photo counts are not", () => {
  const { rerender } = render(<OpeningProjectsDialog projects={[
    row("SARAH XAVIER", "preparing", 3, 70),
    row("YUELSON RODRIGO", "starting"),
  ]} />);
  const status = screen.getByRole("status");
  expect(status).toHaveAttribute("aria-live", "polite");
  expect(status).toHaveTextContent("");

  rerender(<OpeningProjectsDialog projects={[
    row("SARAH XAVIER", "preparing", 4, 70),
    row("YUELSON RODRIGO", "preparing", 1, 40),
  ]} />);
  expect(status).toHaveTextContent("");

  rerender(<OpeningProjectsDialog projects={[
    row("SARAH XAVIER", "ready", 70, 70),
    row("YUELSON RODRIGO", "preparing", 2, 40),
  ]} />);
  expect(status).toHaveTextContent("SARAH XAVIER: Pronto.");

  rerender(<OpeningProjectsDialog projects={[
    row("SARAH XAVIER", "ready", 70, 70),
    row("YUELSON RODRIGO", "failed", 2, 40),
  ]} />);
  expect(status).toHaveTextContent("YUELSON RODRIGO: Não abriu.");
});
