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
  const variant = parameters.get("recents");
  if (variant === "empty") return [];
  if (["single", "missing", "loading"].includes(variant ?? "")) {
    return populatedRecentProjects.slice(0, 1);
  }
  return variant === "mixed"
    ? populatedRecentProjects.slice(0, 4)
    : populatedRecentProjects;
}

export function welcomePreviewFirstSheet(
  parameters: URLSearchParams,
  id: string,
): RecentProjectFirstSheet | null {
  const variant = parameters.get("recents");
  if (variant === "missing" || (variant === "mixed" && id === "p4")) return null;
  const source = representativeProjection.composition.sheets[0];
  const sheet = structuredClone(source);
  if (variant === "mixed" && (id === "p2" || id === "p3")) {
    const rgb = id === "p2" ? "#FFFFFF" : "#eae7df";
    sheet.base.rgb = rgb;
    sheet.backgrounds = [{
      kind: "color",
      rgb,
      drawRect: { ...sheet.base.drawRect },
    }];
    sheet.frames = [];
    sheet.overlays = [];
  }
  if (variant === "single") {
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
    mediaPreviewUrls: sheet.frames.length > 0 ? {
      "media-001": `/src/test/dev-media/${id === "p2" ? "retrato" : "serra-amanhecer"}.svg`,
    } : {},
  };
}
