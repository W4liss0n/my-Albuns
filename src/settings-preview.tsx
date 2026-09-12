import React from "react";
import ReactDOM from "react-dom/client";
import { SettingsWindow } from "./settings/SettingsWindow";
import { photoshopSettingsPreview } from "./test/photoshopPreview";
import type { CacheSettingsPort } from "./application/cacheSettings";
import "./ui/theme.css";
import "./ui/ui.css";

const parameters = new URLSearchParams(window.location.search);
let cache = { occupiedBytes: 104857600, releasableBytes: 41943040, clearAllScheduled: false };
const cachePort: CacheSettingsPort = {
  status: async () => cache,
  freeClosedProjects: async () => {
    const freedBytes = cache.releasableBytes;
    cache = { ...cache, occupiedBytes: cache.occupiedBytes - freedBytes, releasableBytes: 0 };
    return { freedBytes };
  },
  clearAll: async () => { cache = { ...cache, clearAllScheduled: true }; return { kind: "scheduled" }; },
};
const photoshopPort = photoshopSettingsPreview(parameters.get("state"));
ReactDOM.createRoot(document.getElementById("root")!).render(<React.StrictMode>
  <SettingsWindow photoshopPort={photoshopPort} cachePort={cachePort}
    initialSection={parameters.get("section") === "performance" ? "performance" : "photoshop"}
    close={() => { document.body.dataset.settingsClosed = "true"; }} />
</React.StrictMode>);
