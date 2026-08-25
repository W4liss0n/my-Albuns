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
        mediaPreviews={mediaPreviews}
        mediaUsage={mediaUsage}
        onFillPhoto={() => undefined}
        preferences={{ kind: "local" }}
      />
    </main>
  </React.StrictMode>,
);
