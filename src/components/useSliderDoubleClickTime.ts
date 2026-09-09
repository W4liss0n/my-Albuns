import { useEffect, useRef, useState } from "react";
import type { ProjectCorePort } from "../application/projectPorts";

export function useSliderDoubleClickTime(projectId: string, port: ProjectCorePort, onError: (message: string) => void) {
  const [milliseconds, setMilliseconds] = useState<number | null>(null);
  const reportError = useRef(onError);
  reportError.current = onError;
  useEffect(() => {
    let active = true;
    let request = 0;
    const read = () => {
      const serial = ++request;
      setMilliseconds(null);
      void port.readSliderDoubleClickTime().then((value) => {
        if (!Number.isInteger(value) || value <= 0 || value > 5_000) {
          throw new Error("Não foi possível consultar o intervalo de dois cliques do Windows.");
        }
        if (active && serial === request) setMilliseconds(value);
      }).catch((error: unknown) => {
        if (active && serial === request) reportError.current(error instanceof Error ? error.message : String(error));
      });
    };
    read();
    window.addEventListener("focus", read);
    return () => { active = false; window.removeEventListener("focus", read); };
  }, [projectId, port]);
  return milliseconds;
}
