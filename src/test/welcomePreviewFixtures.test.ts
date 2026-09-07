import { expect, test } from "vitest";

import { welcomePreviewRecentProjects } from "./welcomePreviewFixtures";

test("selects no recent Projects for the explicit empty welcome state", () => {
  expect(
    welcomePreviewRecentProjects(new URLSearchParams("recents=empty")),
  ).toEqual([]);
});

test("keeps the existing populated welcome fixture by default", () => {
  expect(
    welcomePreviewRecentProjects(new URLSearchParams()).map(({ name }) => name),
  ).toEqual([
    "Formatura Medicina 2026 — Turma B",
    "Casamento Marina & Téo",
    "Ensaio Helena — 6 meses",
    "15 anos Beatriz",
    "Corporativo Vetra — relatório anual",
    "Batizado Antônio",
    "Retrospectiva Estúdio 2025",
  ]);
});
