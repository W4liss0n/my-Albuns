import type { RecentProjectSummary, RecentProjectFirstSheet } from "../global/application/globalProjectPort";
import { representativeProjection } from "./projectFixtures";

// Preview-only data. It must never be read from or written to the user's real
// recent-Projects storage.
const populatedRecentProjects = [
  { id: "p1", name: "Formatura Medicina 2026 — Turma B", lastOpenedAtMs: null },
  { id: "p2", name: "Casamento Marina & Téo", lastOpenedAtMs: null },
  { id: "p3", name: "Ensaio Helena — 6 meses", lastOpenedAtMs: null },
  { id: "p4", name: "15 anos Beatriz", lastOpenedAtMs: null },
  { id: "p5", name: "Corporativo Vetra — relatório anual", lastOpenedAtMs: null },
  { id: "p6", name: "Batizado Antônio", lastOpenedAtMs: null },
  { id: "p7", name: "Retrospectiva Estúdio 2025", lastOpenedAtMs: null },
] satisfies readonly RecentProjectSummary[];

// Fixed local calendar anchor keeps acceptance screenshots stable across runs.
export const welcomeDatesNow = new Date(2026, 8, 22, 16, 0);

export function welcomePreviewRecentProjects(
  parameters: URLSearchParams,
): readonly RecentProjectSummary[] {
  const variant = parameters.get("recents");
  if (variant === "empty") return [];
  if (variant === "long-names") {
    const [first, second, third, fourth] = populatedRecentProjects;
    return [
      {
        ...first,
        name: "Formatura Medicina 2026 — Turma B, cerimônia e comemoração de encerramento",
        lastOpenedAtMs: new Date(2026, 8, 18, 9, 15).getTime(),
      },
      {
        ...second,
        name: "CasamentoMarinaETeoAlbumCompletoDaCerimoniaEFestaComTodosOsConvidados" +
          "PreparativosDaNoivaEDoNoivoNaFazendaSantaClaraComFamiliaEAmigos" +
          "CelebracaoAoPorDoSolJantarPrimeiraDancaEBrindeDosPadrinhos" +
          "RetratosDaViagemEMemoriasEspeciaisDeTodoOFimDeSemana",
        lastOpenedAtMs: new Date(2026, 8, 22, 14, 30).getTime(),
      },
      { ...third, name: "Ensaio de acompanhamento de Helena — memórias dos primeiros seis meses" },
      { ...fourth, name: "15 anos", lastOpenedAtMs: new Date(2026, 8, 21, 9, 15).getTime() },
    ];
  }
  if (variant === "dates") {
    const [today, yesterday, older, unknown] = populatedRecentProjects;
    return [
      { ...today, lastOpenedAtMs: new Date(2026, 8, 22, 14, 30).getTime() },
      { ...yesterday, lastOpenedAtMs: new Date(2026, 8, 21, 9, 15).getTime() },
      { ...older, lastOpenedAtMs: new Date(2026, 8, 18, 9, 15).getTime() },
      unknown,
    ];
  }
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
