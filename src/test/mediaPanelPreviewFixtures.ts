import type { MediaPreview } from "../application/projectPorts";
import type { MediaCatalogItem, MediaUsage } from "../domain/project";
import campoDouradoUrl from "./dev-media/campo-dourado.svg";
import cidadeUrl from "./dev-media/cidade.svg";
import detalheUrl from "./dev-media/detalhe.svg";
import festaUrl from "./dev-media/festa.svg";
import praiaUrl from "./dev-media/praia.svg";
import retratoCampoUrl from "./dev-media/retrato-campo.svg";
import retratoUrl from "./dev-media/retrato.svg";
import serraAmanhecerUrl from "./dev-media/serra-amanhecer.svg";
import serraNevoaUrl from "./dev-media/serra-nevoa.svg";

// Preview-only data. This catalog simulates imported Fotos and must not be
// used by any production entry point or persisted in a real Projeto.
const importedPhotos = [
  photo({
    id: "test-media-001",
    name: "CAS_2043.jpg",
    sourceWidthPx: 6_000,
    sourceHeightPx: 4_000,
    palette: ["#28384a", "#c47f52", "#f1c678"],
    previewUrl: serraAmanhecerUrl,
    usageCount: 2,
  }),
  photo({
    id: "test-media-002",
    name: "CAS_2046.jpg",
    sourceWidthPx: 6_000,
    sourceHeightPx: 4_000,
    palette: ["#273847", "#71858b", "#d7ad6e"],
    previewUrl: serraNevoaUrl,
    usageCount: 0,
  }),
  photo({
    id: "test-media-003",
    name: "EST_2409.jpg",
    sourceWidthPx: 4_000,
    sourceHeightPx: 6_000,
    palette: ["#43523f", "#a3ad74", "#e7d9bd"],
    previewUrl: retratoCampoUrl,
    usageCount: 1,
  }),
  photo({
    id: "test-media-004",
    name: "COL_2412.jpg",
    sourceWidthPx: 6_000,
    sourceHeightPx: 4_000,
    palette: ["#9b7c4e", "#d2af62", "#ebdfc3"],
    previewUrl: campoDouradoUrl,
    usageCount: 0,
  }),
  photo({
    id: "test-media-005",
    name: "PRA_2415.jpg",
    sourceWidthPx: 6_000,
    sourceHeightPx: 4_000,
    palette: ["#1e526b", "#68a8b2", "#e9c78c"],
    previewUrl: praiaUrl,
    usageCount: 4,
  }),
  photo({
    id: "test-media-006",
    name: "RET_2418.jpg",
    sourceWidthPx: 4_000,
    sourceHeightPx: 6_000,
    palette: ["#4f342e", "#a96f58", "#e5c4a6"],
    previewUrl: retratoUrl,
    usageCount: 0,
  }),
  photo({
    id: "test-media-007",
    name: "BAI_2421.jpg",
    sourceWidthPx: 6_000,
    sourceHeightPx: 4_000,
    palette: ["#1f1b2d", "#875b7b", "#d6a36d"],
    previewUrl: festaUrl,
    usageCount: 3,
  }),
  photo({
    id: "test-media-008",
    name: "DET_2424.jpg",
    sourceWidthPx: 4_000,
    sourceHeightPx: 4_000,
    palette: ["#55624e", "#a5aa88", "#eee6d5"],
    previewUrl: detalheUrl,
    usageCount: 0,
  }),
  photo({
    id: "test-media-009",
    name: "URB_2427.jpg",
    sourceWidthPx: 6_000,
    sourceHeightPx: 4_000,
    palette: ["#303b47", "#778896", "#c9bca7"],
    previewUrl: cidadeUrl,
    usageCount: 1,
  }),
  photo({
    id: "test-media-010",
    name: "PRA_2430.jpg",
    sourceWidthPx: 6_000,
    sourceHeightPx: 4_000,
    palette: ["#234d62", "#5e98a5", "#dcb777"],
    previewUrl: praiaUrl,
    usageCount: 0,
  }),
];

const mediaItems: readonly MediaCatalogItem[] = importedPhotos.map(
  ({ media }) => media,
);

const mediaPreviews = Object.fromEntries(
  importedPhotos.map(({ media, previewUrl }) => [
    media.id,
    {
      mediaId: media.id,
      state: "ready",
      url: previewUrl,
    } satisfies MediaPreview,
  ]),
);

const mediaUsage: readonly MediaUsage[] = importedPhotos.map(
  ({ media, usageCount }) => ({
    mediaId: media.id,
    count: usageCount,
  }),
);

export const mediaPanelPreviewFixture = {
  mediaItems,
  mediaPreviews,
  mediaUsage,
} as const;

interface PreviewPhotoInput extends Omit<MediaCatalogItem, "kind"> {
  previewUrl: string;
  usageCount: number;
}

function photo({ previewUrl, usageCount, ...media }: PreviewPhotoInput) {
  return {
    media: {
      ...media,
      kind: "photo",
    } satisfies MediaCatalogItem,
    previewUrl,
    usageCount,
  };
}
