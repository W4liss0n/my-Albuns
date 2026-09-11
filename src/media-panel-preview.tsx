import React from "react";
import ReactDOM from "react-dom/client";

import "./ui/theme.css";
import "./ui/ui.css";
import { MediaPanel } from "./components/MediaPanel";
import { mediaPanelPreviewFixture } from "./test/mediaPanelPreviewFixtures";
import "./media-panel-preview.css";
import type { MediaFileInfo } from "./application/projectPorts";

const { mediaItems, mediaPreviews, mediaUsage } = mediaPanelPreviewFixture;
const parameters = new URLSearchParams(window.location.search);
const mediaFiles: Record<string, MediaFileInfo> = parameters.has("files") ? Object.fromEntries(mediaItems.map((media, index) => [media.id, {
  mediaId: media.id, state: index === 2 || index === 10 ? "absent" : "available",
  createdAtMs: index === 5 ? null : 1_780_000_000_000 + (mediaItems.length - index) * 1000,
  modifiedAtMs: index === 5 ? null : 1_780_000_000_000 + index * 1000,
}])) : {};
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
        onRetryUnavailableMedia={async () => undefined}
        preferences={{ kind: "local", initialThumbnailSize: parameters.get("files") === "dates" ? 96 : undefined,
          initial: parameters.get("files") === "dates" ? {
            photo: { sortKey: "createdAt", sortDirection: "descending", usageFilter: "all" },
          } : undefined }}
        previewSource={{ kind: "static", previews: mediaPreviews }}
      />
    </main>
  </React.StrictMode>,
);
