export interface GraphicsLimits {
  maxTextureSizePx: number;
  maxRenderbufferSizePx: number;
  maxTextureImageUnits: number;
}

export type GraphicsUnavailableCode =
  | "webgl2_unavailable"
  | "hardware_unconfirmed"
  | "software_renderer"
  | "canvas_initialization_failed"
  | "context_restore_failed";

export type GraphicsDiagnostic =
  | {
      supported: true;
      renderer: string;
      reason: string;
      limits: GraphicsLimits;
    }
  | {
      supported: false;
      code: GraphicsUnavailableCode;
      renderer: string;
      reason: string;
      limits: GraphicsLimits | null;
    };

export type GraphicsProbe = () => GraphicsDiagnostic;
