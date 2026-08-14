import { Minus, Square, X } from "lucide-react";

import { AppIcon } from "./AppIcon";
import { BrandWordmark } from "./BrandWordmark";
import { useWindowControls } from "./WindowControlsContext";

interface ApplicationHeaderProps {
  controls?: "all" | "close" | "none";
  context?: string;
  metadata?: string;
  status?: string;
}

function runWindowAction(action: () => Promise<void> | void) {
  try {
    void Promise.resolve(action()).catch(() => undefined);
  } catch {
    // Browser previews do not expose a native Tauri window.
  }
}

export function ApplicationHeader({
  controls = "all",
  context,
  metadata,
  status,
}: ApplicationHeaderProps) {
  const windowControls = useWindowControls();

  return (
    <header
      aria-label="Barra da janela"
      className="ui-application-header"
      data-window-controls={controls}
    >
      <div
        aria-hidden="true"
        className="ui-titlebar-drag-region"
        data-tauri-drag-region
      />
      <span className="ui-application-header__identity">
        <BrandWordmark compact />
        {context ? (
          <>
            <span aria-hidden="true" className="ui-header-separator">
              ·
            </span>
            <strong>{context}</strong>
          </>
        ) : null}
        {metadata ? <small>{metadata}</small> : null}
      </span>
      <span
        aria-hidden={status ? undefined : "true"}
        className="ui-application-header__status"
      >
        {status}
      </span>
      {controls !== "none" ? (
        <span
          aria-label="Controles da janela"
          className="ui-window-controls"
          role="group"
        >
          {controls === "all" ? (
            <>
              <button
                aria-label="Minimizar janela"
                className="ui-window-control ui-window-control--minimize"
                onClick={() => runWindowAction(windowControls.minimize)}
                title="Minimizar"
                type="button"
              >
                <AppIcon icon={Minus} size={12} />
              </button>
              <button
                aria-label="Maximizar ou restaurar janela"
                className="ui-window-control ui-window-control--maximize"
                onClick={() =>
                  runWindowAction(windowControls.toggleMaximize)
                }
                title="Maximizar ou restaurar"
                type="button"
              >
                <AppIcon icon={Square} size={12} />
              </button>
            </>
          ) : null}
          <button
            aria-label="Fechar janela"
            className="ui-window-control ui-window-control--close"
            onClick={() => runWindowAction(windowControls.close)}
            title="Fechar"
            type="button"
          >
            <AppIcon icon={X} size={14} />
          </button>
        </span>
      ) : null}
    </header>
  );
}
