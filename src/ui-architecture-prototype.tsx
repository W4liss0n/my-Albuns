import React from "react";
import ReactDOM from "react-dom/client";

import {
  UiArchitecturePrototype,
  type UiArchitecturePrototypeEditorMode,
  type UiArchitecturePrototypeView,
} from "./prototypes/UiArchitecturePrototype";
import "./ui/theme.css";
import "./prototypes/UiArchitecturePrototype.css";

const search = new URLSearchParams(window.location.search);
const requestedView = search.get("view");
const requestedEditorMode = search.get("mode");
const initialView: UiArchitecturePrototypeView =
  requestedView === "editor" ? "editor" : "map";
const initialEditorMode: UiArchitecturePrototypeEditorMode =
  requestedEditorMode === "normal" ? "normal" : "edit";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <UiArchitecturePrototype
      initialEditorMode={initialEditorMode}
      initialView={initialView}
    />
  </React.StrictMode>,
);
