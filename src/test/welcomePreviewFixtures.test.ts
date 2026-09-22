import { expect, test } from "vitest";

import { welcomePreviewFirstSheet, welcomePreviewRecentProjects } from "./welcomePreviewFixtures";

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

test("mixed recent Projects include photo, white, warm-neutral, and unavailable previews", () => {
  const parameters = new URLSearchParams("recents=mixed");
  const ids = welcomePreviewRecentProjects(parameters).map(({ id }) => id);
  expect(ids).toEqual(["p1", "p2", "p3", "p4"]);
  const photo = welcomePreviewFirstSheet(parameters, "p1");
  const white = welcomePreviewFirstSheet(parameters, "p2");
  const warm = welcomePreviewFirstSheet(parameters, "p3");
  expect(photo?.sheet.frames.length).toBeGreaterThan(0);
  expect(photo?.mediaPreviewUrls).toHaveProperty("media-001");
  expect(white?.sheet.base.rgb).toBe("#FFFFFF");
  expect(white?.sheet.frames).toEqual([]);
  expect(warm?.sheet.base.rgb).toBe("#eae7df");
  expect(warm?.sheet.frames).toEqual([]);
  expect(welcomePreviewFirstSheet(parameters, "p4")).toBeNull();
});

test("loading state keeps one recent Project card", () => {
  expect(welcomePreviewRecentProjects(new URLSearchParams("recents=loading")))
    .toHaveLength(1);
});
