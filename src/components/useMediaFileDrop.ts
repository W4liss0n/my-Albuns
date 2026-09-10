import { useEffect, useRef, useState, type RefObject } from "react";
import type { MediaDropPort, MediaImportSelection } from "../application/projectPorts";

export function useMediaFileDrop({ port, host, hidden, disabled, mediaKind, onImport }: {
  port?: MediaDropPort;
  host: RefObject<HTMLElement | null>;
  hidden: boolean;
  disabled: boolean;
  mediaKind: MediaImportSelection["mediaKind"];
  onImport(selection: MediaImportSelection): void;
}) {
  const [over, setOver] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const current = useRef({ hidden, disabled, mediaKind, onImport });
  current.current = { hidden, disabled, mediaKind, onImport };
  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    setError(null);
    if (!port) return;
    void port.subscribe((event) => {
      if (!active) return;
      const latest = current.current;
      const bounds = host.current?.getBoundingClientRect();
      const inside = event.kind !== "leave" && !latest.hidden && !latest.disabled && bounds &&
        event.x >= bounds.left && event.x <= bounds.right && event.y >= bounds.top && event.y <= bounds.bottom;
      setOver(Boolean(inside && event.kind === "over"));
      if (inside && event.kind === "drop" && event.paths.length > 0) {
        latest.onImport({ mediaKind: latest.mediaKind, source: { kind: "drop", paths: event.paths } });
      }
    }).then((stop) => { if (active) unsubscribe = stop; else stop(); }, () => {
      if (active) setError("Não foi possível receber arquivos arrastados. Use Importar.");
    });
    return () => { active = false; unsubscribe?.(); };
  }, [port, host]);
  return { over: over && !hidden && !disabled, error };
}
