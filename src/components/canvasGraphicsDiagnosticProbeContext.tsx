import {
  createContext,
  type PropsWithChildren,
  useContext,
} from "react";

import type { GraphicsDiagnostic } from "../application/graphics";

export type CanvasGraphicsDiagnosticProbe = (
  canvas: HTMLCanvasElement,
) => GraphicsDiagnostic;

const CanvasGraphicsDiagnosticProbeContext =
  createContext<CanvasGraphicsDiagnosticProbe | null>(null);

export function CanvasGraphicsDiagnosticProbeProvider({
  children,
  probe,
}: PropsWithChildren<{ probe: CanvasGraphicsDiagnosticProbe }>) {
  return (
    <CanvasGraphicsDiagnosticProbeContext value={probe}>
      {children}
    </CanvasGraphicsDiagnosticProbeContext>
  );
}

export function useCanvasGraphicsDiagnosticProbe() {
  const probe = useContext(CanvasGraphicsDiagnosticProbeContext);
  if (!probe) {
    throw new Error(
      "CanvasGraphicsDiagnosticProbe não foi definido na raiz de composição.",
    );
  }
  return probe;
}
