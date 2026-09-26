import { expect, test } from "vitest";

import { welcomeDatesNow, welcomePreviewFirstSheet, welcomePreviewRecentProjects } from "./welcomePreviewFixtures";
import { recentProjectOpeningTime } from "../global/recentProjectOpeningTime";

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

test("date preview has a fixed local clock and a legacy card without a timestamp", () => {
  const dates = welcomePreviewRecentProjects(new URLSearchParams("recents=dates"));
  expect(dates).toHaveLength(4);
  expect(dates.slice(0, 3).map(({ lastOpenedAtMs }) =>
    recentProjectOpeningTime(lastOpenedAtMs, welcomeDatesNow)?.fullLabel,
  )).toEqual(["Hoje às 14:30", "Ontem às 09:15", "18/09/2026 às 09:15"]);
  expect(dates[3].lastOpenedAtMs).toBeNull();
});

test("long-name preview covers dates, an unbroken name and a legacy card", () => {
  const projects = welcomePreviewRecentProjects(new URLSearchParams("recents=long-names"));
  expect(projects).toHaveLength(4);
  expect(projects[0].name).toContain("cerimônia e comemoração");
  expect(projects[1].name).not.toContain(" ");
  expect(projects[2].lastOpenedAtMs).toBeNull();
  expect(projects[3].name).toBe("15 anos");
  expect(projects.map(({ lastOpenedAtMs }) =>
    recentProjectOpeningTime(lastOpenedAtMs, welcomeDatesNow)?.label ?? null,
  )).toEqual([
    "Aberto em 18/09/2026 às 09:15",
    "Aberto hoje às 14:30",
    null,
    "Aberto ontem às 09:15",
  ]);
});
