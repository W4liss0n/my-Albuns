import { useEffect, useState } from "react";
import type { PhotoshopSettingsPort, SettingsSection } from "../application/photoshop";
import type { CacheSettingsPort } from "../application/cacheSettings";
import { ActionButton, ApplicationHeader } from "../ui";
import { PhotoshopSettings } from "./PhotoshopSettings";
import { CacheSettings } from "./CacheSettings";
import "./SettingsWindow.css";

export interface SettingsWindowProps {
  initialSection?: SettingsSection;
  photoshopPort: PhotoshopSettingsPort;
  cachePort: CacheSettingsPort;
  close(): void;
  onSectionRequest?(listener: (section: SettingsSection) => void): Promise<() => void>;
}

export function SettingsWindow({ initialSection = "performance", photoshopPort, cachePort, close, onSectionRequest }: SettingsWindowProps) {
  const [section, setSection] = useState<SettingsSection>(initialSection);
  useEffect(() => {
    if (!onSectionRequest) return;
    let disposed = false;
    let release: (() => void) | undefined;
    void onSectionRequest(setSection).then((unlisten) => { if (disposed) unlisten(); else release = unlisten; }).catch(() => undefined);
    return () => { disposed = true; release?.(); };
  }, [onSectionRequest]);
  return <div className="application-settings ui-chrome-selection-scope">
    <ApplicationHeader context="Configurações" controls="close" />
    <div role="tablist" aria-label="Configurações" className="application-settings-tabs">
      {(["performance", "photoshop"] as const).map((value, index, sections) => <button key={value} type="button" role="tab"
        id={`settings-tab-${value}`} aria-controls={`settings-panel-${value}`} aria-selected={section === value} tabIndex={section === value ? 0 : -1}
        onClick={() => setSection(value)} onKeyDown={(event) => {
          if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
          event.preventDefault();
          const target = sections[event.key === "Home" ? 0 : event.key === "End" ? sections.length - 1 : (index + (event.key === "ArrowRight" ? 1 : -1) + sections.length) % sections.length];
          setSection(target); document.getElementById(`settings-tab-${target}`)?.focus();
        }}>{value === "performance" ? "Desempenho" : "Photoshop"}</button>)}
    </div>
    <main>
      <div role="tabpanel" id="settings-panel-performance" aria-labelledby="settings-tab-performance" hidden={section !== "performance"}>
        <CacheSettings port={cachePort} />
      </div>
      <div role="tabpanel" id="settings-panel-photoshop" aria-labelledby="settings-tab-photoshop" hidden={section !== "photoshop"}>
        <PhotoshopSettings port={photoshopPort} />
      </div>
    </main>
    <footer><ActionButton onClick={close}>Fechar</ActionButton></footer>
  </div>;
}
