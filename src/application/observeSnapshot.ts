/** Installs live delivery before reading initial state and owns the whole lifetime. */
export function observeSnapshot<T>({ subscribe, read, receive }: {
  subscribe(receive: (value: T) => void): Promise<() => void>;
  read(): Promise<T | null>;
  receive(value: T | null): void;
}): { ready: Promise<void>; dispose(): void } {
  let active = true;
  let receivedLive = false;
  let release: (() => void) | undefined;
  const dispose = () => {
    active = false;
    const stop = release;
    release = undefined;
    stop?.();
  };
  const ready = (async () => {
    try {
      const stop = await subscribe(value => {
        if (!active) return;
        receivedLive = true;
        receive(value);
      });
      if (!active) { stop(); return; }
      release = stop;
      const initial = await read();
      if (active && !receivedLive) receive(initial);
    } catch (error) {
      if (!active) return;
      dispose();
      throw error;
    }
  })();
  return { ready, dispose };
}
