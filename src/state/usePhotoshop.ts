import { useCallback, useEffect, useRef, useState } from "react";
import { PhotoshopError, photoshopErrorMessage, type PhotoshopPhotoTarget, type PhotoshopPort, type PhotoshopStatus, type SettingsSection } from "../application/photoshop";

export function usePhotoshop(port: PhotoshopPort | undefined) {
  const [status, setStatus] = useState<PhotoshopStatus | null>(null);
  const [error, setError] = useState<{ message: string; configure: boolean } | null>(null);
  const [opening, setOpening] = useState(false);
  const sequence = useRef(0);
  const running = useRef(false);
  const alive = useRef(false);

  const refresh = useCallback(async () => {
    if (!port) return;
    const request = ++sequence.current;
    try {
      const loaded = await port.status();
      if (alive.current && request === sequence.current) setStatus(loaded);
    } catch {
      if (alive.current && request === sequence.current) setStatus(null);
    }
  }, [port]);

  useEffect(() => {
    alive.current = true;
    void refresh();
    window.addEventListener("focus", refresh);
    return () => { alive.current = false; sequence.current += 1; window.removeEventListener("focus", refresh); };
  }, [refresh]);

  const open = useCallback(async (target: PhotoshopPhotoTarget) => {
    if (!port || running.current) return;
    running.current = true;
    setOpening(true);
    setError(null);
    try { await port.openPhoto(target); }
    catch (error) {
      if (alive.current) setError({
        message: photoshopErrorMessage(error),
        configure: error instanceof PhotoshopError && ["installation_unavailable", "launch_failed"].includes(error.code),
      });
    } finally {
      running.current = false;
      if (alive.current) setOpening(false);
    }
  }, [port]);

  const openSettings = useCallback(async (section: SettingsSection = "photoshop") => {
    if (!port) return;
    try { await port.openSettings(section); }
    catch (error) { if (alive.current) setError({ message: photoshopErrorMessage(error), configure: false }); }
  }, [port]);

  return { available: status?.selectedInstallationId != null, opening, open, openSettings, error, dismissError: () => setError(null) };
}
