import type { RecentProjectSummary } from "../global/application/globalProjectPort";

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
  return parameters.get("recents") === "empty" ? [] : populatedRecentProjects;
}
