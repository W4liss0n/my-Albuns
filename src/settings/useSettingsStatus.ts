import { useEffect, useRef, useState } from "react";

/** Each settings section owns its requests; only their ordering policy is shared. */
export function useSettingsStatus<Status>(
  port: { status(): Promise<Status> },
  readErrorMessage: (error: unknown) => string,
  updateErrorMessage = readErrorMessage,
) {
  const [status, setStatus] = useState<Status | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const lifetime = useRef({ active: false, sequence: 0, running: false });

  useEffect(() => {
    const current = { active: true, sequence: 0, running: false };
    lifetime.current = current;
    setPending(false);
    const refresh = async () => {
      if (current.running) return;
      const request = ++current.sequence;
      try {
        const next = await port.status();
        if (current.active && request === current.sequence) {
          setStatus(next);
          setError(null);
        }
      } catch (error) {
        if (current.active && request === current.sequence) setError(readErrorMessage(error));
      }
    };
    void refresh();
    window.addEventListener("focus", refresh);
    return () => {
      current.active = false;
      window.removeEventListener("focus", refresh);
    };
  }, [port, readErrorMessage]);

  async function update<Result>(operation: () => Promise<Result>, receive: (result: Result) => Status | null) {
    const current = lifetime.current;
    if (!current.active || current.running) return;
    current.running = true;
    const request = ++current.sequence;
    setPending(true);
    setError(null);
    try {
      const result = await operation();
      if (current.active && request === current.sequence) {
        const next = receive(result);
        if (next !== null) setStatus(next);
      }
    } catch (error) {
      if (current.active && request === current.sequence) setError(updateErrorMessage(error));
    } finally {
      current.running = false;
      if (current.active && request === current.sequence) setPending(false);
    }
  }

  return { status, pending, error, update };
}
