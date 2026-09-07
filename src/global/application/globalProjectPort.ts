export interface ProjectLaunchFailure {
  code: string;
  stage?: string;
  message: string;
  action?: string;
}

export type ProjectLaunchOutcome =
  | { status: "opened" }
  | { status: "focused" }
  | { status: "cancelled" }
  | { status: "failed"; error: ProjectLaunchFailure };

export type OpenProjectFailure = ProjectLaunchFailure;
export type OpenProjectOutcome = ProjectLaunchOutcome;

export type OpeningExternalCopyDecision = "saveCopyAs" | "cancel";

export type NewProjectOperationalFailureContext =
  | "configurationValidation"
  | "decorativeSelection"
  | "projectCreation";

export interface NewProjectOperationalFailure {
  context: NewProjectOperationalFailureContext;
  error: ProjectLaunchFailure;
}

export type ProjectFailureDialogContext =
  | "projectOpening"
  | NewProjectOperationalFailureContext;

export interface ProjectFailureDialogRequest {
  context: ProjectFailureDialogContext;
  error: ProjectLaunchFailure;
}

export interface ProjectFailureDialogPort {
  present(failure: ProjectFailureDialogRequest): Promise<void>;
}

export type ProjectDisplayUnit = "mm" | "cm" | "in";
export type ProjectEndSheetFormat = "double" | "singlePage";

export interface NewProjectConfiguration {
  document: {
    displayUnit: ProjectDisplayUnit;
    sheetWidthUm: number;
    sheetHeightUm: number;
    dpi: number;
    bleedUm: number;
    safetyUm: number;
  };
  structure: {
    sheetCount: number;
    firstSheet: ProjectEndSheetFormat;
    lastSheet: ProjectEndSheetFormat;
  };
}

export interface ProvisionalDecorativeSelection {
  selectionId: string;
  displayName: string;
  previewUrl: string;
}

export type ProvisionalDecorativeSelectionOutcome =
  | {
      status: "selected";
      selection: ProvisionalDecorativeSelection;
    }
  | { status: "cancelled" }
  | { status: "failed"; error: ProjectLaunchFailure };

export type InitialBackgroundContent =
  | { kind: "color"; rgb: string }
  | { kind: "image"; selectionId: string };

export type InitialOverlayContent =
  | { kind: "image"; selectionId: string }
  | null;

export type InitialFrameBorder =
  | { kind: "none" }
  | { kind: "solid"; rgb: string; widthUm: number };

export interface InitialVisualDefaults {
  background: ScopedValue<InitialBackgroundContent>;
  overlay: ScopedValue<InitialOverlayContent>;
  frameBorder: InitialFrameBorder;
}

export interface NewProjectCreationConfiguration
  extends NewProjectConfiguration {
  visualDefaults: InitialVisualDefaults;
}

export const PROJECT_CONFIGURATION_VALIDATION_CODES = [
  "sheetWidthNotPositive",
  "sheetWidthAboveSafeInteger",
  "sheetWidthNotEven",
  "sheetWidthRasterOutOfRange",
  "sheetHeightNotPositive",
  "sheetHeightAboveSafeInteger",
  "sheetHeightRasterOutOfRange",
  "sheetDimensionsNotProportional",
  "dpiOutOfRange",
  "sheetCountTooSmall",
  "bleedNegative",
  "bleedAboveSafeInteger",
  "bleedEliminatesCutArea",
  "safetyNegative",
  "safetyAboveSafeInteger",
  "safetyEliminatesSafeArea",
] as const;

export type ProjectConfigurationValidationCode =
  (typeof PROJECT_CONFIGURATION_VALIDATION_CODES)[number];

export type ProjectConfigurationValidationOutcome =
  | { status: "valid" }
  | {
      status: "invalid";
      errors: readonly ProjectConfigurationValidationCode[];
    }
  | { status: "failed"; error: ProjectLaunchFailure };

export interface RecentProjectSummary {
  id: string;
  name: string;
}

export interface GlobalProjectPort {
  onActivationTerminal(
    listener: (outcome: ProjectLaunchOutcome) => void,
  ): Promise<() => void>;
  completeGraphicsGate(
    supported: boolean,
  ): Promise<ProjectLaunchOutcome | null>;
  openProject(): Promise<OpenProjectOutcome>;
  listRecentProjects(): Promise<readonly RecentProjectSummary[]>;
  openRecentProject(id: string): Promise<OpenProjectOutcome>;
  startupOpenFailure(): Promise<OpenProjectFailure | null>;
}

export interface NewProjectPort {
  validateProjectConfiguration(
    configuration: NewProjectConfiguration,
  ): Promise<ProjectConfigurationValidationOutcome>;
  createProject(
    configuration: NewProjectCreationConfiguration,
  ): Promise<ProjectLaunchOutcome>;
  chooseProvisionalDecorative(): Promise<ProvisionalDecorativeSelectionOutcome>;
  releaseProvisionalDecorative(selectionId: string): Promise<void>;
  clearProvisionalDecoratives(): Promise<void>;
}
import type { ScopedValue } from "../../application/scopedValues";
