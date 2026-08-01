import ReactDOM from "react-dom/client";

import { tauriLogger } from "../platform/tauriLogger";
import { GlobalShell } from "./GlobalShell";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <GlobalShell logger={tauriLogger} />,
);
