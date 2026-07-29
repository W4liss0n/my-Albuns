import { createContext, useContext, type ReactNode } from "react";

import {
  silentLogger,
  type Logger,
} from "../application/logging";

const LoggingContext = createContext<Logger>(silentLogger);

export function LoggingProvider({
  logger,
  children,
}: {
  logger: Logger;
  children: ReactNode;
}) {
  return (
    <LoggingContext.Provider value={logger}>
      {children}
    </LoggingContext.Provider>
  );
}

export function useLogger() {
  return useContext(LoggingContext);
}
