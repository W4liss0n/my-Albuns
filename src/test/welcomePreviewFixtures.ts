import type { RecentProjectSummary, RecentProjectFirstSheet } from "../global/application/globalProjectPort";
import { representativeProjection } from "./projectFixtures";

// Preview-only data. It must never be read from or written to the user's real
// recent-Projects storage.
const populatedRecentProjects = [
  { id: "p1", name: "Formatura Medicina 2026 — Turma B" },
  { id: "p2", name: "Casamento Marina & Téo" },
  { id: "p3", name: "Ensaio Helena — 6 meses" },
  { id: "p4", name: "15 anos Beatriz" },
  { id: "p5", name: "Corporativo Vetra — relatório anual" },
  { id: "p6", name: "Batizado Antônio" },
  { id: "p7", name: "Retrospectiva Estúdio 2025" },
] satisfies readonly RecentProjectSummary[];

export function welcomePreviewRecentProjects(
  parameters: URLSearchParams,
): readonly RecentProjectSummary[] {
  return parameters.get("recents") === "empty" ? [] :
    ["single", "missing"].includes(parameters.get("recents") ?? "")
      ? populatedRecentProjects.slice(0, 1) : populatedRecentProjects;
}

export function welcomePreviewFirstSheet(
  parameters: URLSearchParams,
  id: string,
): RecentProjectFirstSheet | null {
  if (parameters.get("recents") === "missing") return null;
  const source = representativeProjection.composition.sheets[0];
  const sheet = structuredClone(source);
  if (parameters.get("recents") === "single") {
    sheet.activeSides = "left";
    sheet.widthUm = source.widthUm / 2;
    sheet.base.drawRect.width = sheet.widthUm;
    sheet.frames = sheet.frames.map((frame) => ({
      ...frame,
      clipRect: { ...frame.clipRect, width: Math.min(frame.clipRect.width, 260_000) },
    }));
  }
  return {
    sheet,
    mediaPreviewUrls: {
      "media-001": `/src/test/dev-media/${id === "p2" ? "retrato" : "serra-amanhecer"}.svg`,
    },
  };
}
