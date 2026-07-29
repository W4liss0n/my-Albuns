export interface GraphicsDiagnostic {
  supported: boolean;
  renderer: string;
  reason: string;
}

export type GraphicsProbe = () => GraphicsDiagnostic;
