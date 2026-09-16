import React from "react";
import ReactDOM from "react-dom/client";

import "./ui/theme.css";
import "./ui/ui.css";
import { MediaPanel } from "./components/MediaPanel";
import { mediaPanelPreviewFixture } from "./test/mediaPanelPreviewFixtures";
import "./media-panel-preview.css";
import type { MediaFolder } from "./domain/project";
import type { MediaFileInfo } from "./application/projectPorts";

const { mediaItems, mediaPreviews, mediaUsage } = mediaPanelPreviewFixture;
const parameters = new URLSearchParams(window.location.search);
const mediaFiles: Record<string, MediaFileInfo> = parameters.has("files") ? Object.fromEntries(mediaItems.map((media, index) => [media.id, {
  mediaId: media.id, state: index === 2 || index === 10 ? "absent" : "available",
  createdAtMs: index === 5 ? null : 1_780_000_000_000 + (mediaItems.length - index) * 1000,
  modifiedAtMs: index === 5 ? null : 1_780_000_000_000 + index * 1000,
}])) : {};
const folders: MediaFolder[] = parameters.has("folders") ? [
  { id: "folder-retratos", kind: "photo", name: "Retratos", mediaIds: ["test-media-003", "test-media-006"] },
  { id: "folder-cerimonia", kind: "photo", name: "Cerimônia", mediaIds: ["test-media-001", "test-media-002", "test-media-007"] },
  { id: "folder-externas", kind: "photo", name: "Fotos externas da turma de formandos", mediaIds: ["test-media-004", "test-media-005"] },
  { id: "folder-familias", kind: "photo", name: "Famílias", mediaIds: [] },
  { id: "folder-fundos", kind: "decorative", name: "Fundos", mediaIds: ["test-decorative-001"] },
] : [];
const displayedPreviews = parameters.get("cache") === "missing"
  ? Object.fromEntries(Object.entries(mediaPreviews).filter(([mediaId]) => mediaFiles[mediaId]?.state !== "absent"))
  : mediaPreviews;
const acceptanceSurface =
  new URLSearchParams(window.location.search).get("acceptance") === "editor"
    ? "editor"
    : undefined;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <main
      className="media-panel-preview"
      data-acceptance-surface={acceptanceSurface}
      data-development-preview="imported-media"
    >
      <MediaPanel
        photoshopAvailable={parameters.get("photoshop") === "available"}
        onOpenInPhotoshop={() => undefined}
        mediaFolders={folders}
        onEditMediaFolder={async () => true}
        mediaItems={mediaItems}
        mediaUsage={mediaUsage}
        mediaFiles={mediaFiles}
        onFillPhoto={() => undefined}
        onApplyDecorative={() => undefined}
        onImportMedia={() => undefined}
        onRemoveMedia={() => undefined}
        importPending={new URLSearchParams(window.location.search).get("import") === "pending"}
        onMediaDragChange={() => undefined}
        onRelinkMedia={() => undefined}
        onReplaceMedia={() => undefined}
        onRetryUnavailableMedia={async () => undefined}
        preferences={{ kind: "local", initialThumbnailSize: parameters.get("files") === "dates" ? 96 : undefined,
          initial: parameters.get("files") === "dates" ? {
            photo: { sortKey: "createdAt", sortDirection: "descending", usageFilter: "all" },
          } : undefined }}
        previewSource={{ kind: "static", previews: displayedPreviews }}
      />
    </main>
  </React.StrictMode>,
);
