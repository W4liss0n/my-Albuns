import React from "react";
import ReactDOM from "react-dom/client";
import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { tauriProjectGenerationPort } from "../platform/tauriProjectGenerationPort";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import { WindowControlsProvider } from "../ui/WindowControlsContext";
import { GenerationWindow } from "./GenerationWindow";
import { GenerationProgressWindow } from "./GenerationProgressWindow";
import "../ui/theme.css";
import "../ui/ui.css";

installDesktopWebViewPolicy(document);
const progress = new URLSearchParams(window.location.search).get("surface") === "progress";
const controls = { ...tauriWindowControls, close: progress ? tauriProjectGenerationPort.cancel : tauriProjectGenerationPort.close };
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<React.StrictMode><WindowControlsProvider controls={controls}>{progress ? <GenerationProgressWindow port={tauriProjectGenerationPort} /> : <GenerationWindow port={tauriProjectGenerationPort} />}</WindowControlsProvider></React.StrictMode>);
