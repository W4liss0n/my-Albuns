import React from "react";
import ReactDOM from "react-dom/client";

import "./App.css";
import { MediaPanel } from "./components/MediaPanel";
import { mediaPanelPreviewFixture } from "./test/mediaPanelPreviewFixtures";
import "./media-panel-preview.css";

const { mediaItems, mediaPreviews, mediaUsage } = mediaPanelPreviewFixture;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <main
      className="media-panel-preview"
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
