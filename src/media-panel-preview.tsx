import React from "react";
import ReactDOM from "react-dom/client";

import "./App.css";
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
      />
    </main>
  </React.StrictMode>,
);
