import {
  createContext,
  type ReactNode,
  useContext,
} from "react";

export interface WindowControls {
  close(): Promise<void> | void;
  fitContent(height: number): Promise<void> | void;
  minimize(): Promise<void> | void;
  toggleMaximize(): Promise<void> | void;
}

const browserWindowControls: WindowControls = {
  close: () => undefined,
  fitContent: () => undefined,
  minimize: () => undefined,
  toggleMaximize: () => undefined,
};

const WindowControlsContext = createContext<WindowControls>(
  browserWindowControls,
);

interface WindowControlsProviderProps {
  children: ReactNode;
  controls: WindowControls;
}

export function WindowControlsProvider({
  children,
  controls,
}: WindowControlsProviderProps) {
  return (
    <WindowControlsContext.Provider value={controls}>
      {children}
    </WindowControlsContext.Provider>
  );
}

export function useWindowControls() {
  return useContext(WindowControlsContext);
}
