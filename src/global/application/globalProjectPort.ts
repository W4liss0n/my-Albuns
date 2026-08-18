export interface ProjectLaunchFailure {
  code: string;
  stage?: string;
  message: string;
  action?: string;
}

export type ProjectLaunchOutcome =
  | { status: "opened" }
  | { status: "focused" }
  | { status: "externalCopyNotWritable" }
  | { status: "cancelled" }
  | { status: "failed"; error: ProjectLaunchFailure };

export type OpenProjectFailure = ProjectLaunchFailure;
export type OpenProjectOutcome = ProjectLaunchOutcome;

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

export type InitialScopedContent<T> =
  | { scope: "bothSides"; both: T }
  | { scope: "perSide"; left: T; right: T };

export type InitialFrameBorder =
  | { kind: "none" }
  | { kind: "solid"; rgb: string; widthUm: number };

export interface InitialVisualDefaults {
  background: InitialScopedContent<InitialBackgroundContent>;
  overlay: InitialScopedContent<InitialOverlayContent>;
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
  completeGraphicsGate(
    supported: boolean,
  ): Promise<ProjectLaunchOutcome | null>;
  validateProjectConfiguration(
    configuration: NewProjectConfiguration,
  ): Promise<ProjectConfigurationValidationOutcome>;
  createProject(
    configuration: NewProjectCreationConfiguration,
  ): Promise<ProjectLaunchOutcome>;
  chooseProvisionalDecorative(): Promise<ProvisionalDecorativeSelectionOutcome>;
  releaseProvisionalDecorative(selectionId: string): Promise<void>;
  clearProvisionalDecoratives(): Promise<void>;
  openProject(): Promise<OpenProjectOutcome>;
  saveExternalCopyAs(): Promise<ProjectLaunchOutcome>;
  listRecentProjects(): Promise<readonly RecentProjectSummary[]>;
  openRecentProject(id: string): Promise<OpenProjectOutcome>;
  startupOpenFailure(): Promise<OpenProjectFailure | null>;
}
