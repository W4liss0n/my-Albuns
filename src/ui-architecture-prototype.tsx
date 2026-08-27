import React from "react";
import ReactDOM from "react-dom/client";

import {
  UiArchitecturePrototype,
  type UiArchitecturePrototypeView,
} from "./prototypes/UiArchitecturePrototype";
import "./ui/theme.css";
import "./prototypes/UiArchitecturePrototype.css";

const requestedView = new URLSearchParams(window.location.search).get("view");
const initialView: UiArchitecturePrototypeView =
  requestedView === "editor" ? "editor" : "map";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <UiArchitecturePrototype initialView={initialView} />
  </React.StrictMode>,
);
