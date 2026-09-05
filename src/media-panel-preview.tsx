import React from "react";
import ReactDOM from "react-dom/client";

import "./ui/theme.css";
import "./ui/ui.css";
import { MediaPanel } from "./components/MediaPanel";
import { mediaPanelPreviewFixture } from "./test/mediaPanelPreviewFixtures";
import "./media-panel-preview.css";

const { mediaItems, mediaPreviews, mediaUsage } = mediaPanelPreviewFixture;
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
        onFillPhoto={() => undefined}
        selectedMediaId={null}
        onImportPhoto={() => undefined}
        importPending={new URLSearchParams(window.location.search).get("import") === "pending"}
        importStatus={new URLSearchParams(window.location.search).get("import") === "completed" ? "10 Fotos importadas." : undefined}
        onSelectMedia={() => undefined}
        onPhotoDragStart={() => undefined}
        onPhotoDragEnd={() => undefined}
        onRelinkMedia={() => undefined}
        onRetryUnavailableMedia={async () => undefined}
        preferences={{ kind: "local" }}
        previewSource={{ kind: "static", previews: mediaPreviews }}
      />
    </main>
  </React.StrictMode>,
);
