import React from "react";
import ReactDOM from "react-dom/client";

import "./ui/theme.css";
import "./ui/ui.css";
import { SheetGridPreview } from "./SheetGridPreview";
import "./sheet-grid-preview.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SheetGridPreview />
  </React.StrictMode>,
);
