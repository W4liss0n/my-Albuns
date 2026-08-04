export interface ProjectLaunchFailure {
  code: string;
  stage?: string;
  message: string;
  action?: string;
}

export type ProjectLaunchOutcome =
  | { status: "opened" }
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
  validateProjectConfiguration(
    configuration: NewProjectConfiguration,
  ): Promise<ProjectConfigurationValidationOutcome>;
  createProject(
    configuration: NewProjectConfiguration,
  ): Promise<ProjectLaunchOutcome>;
  openProject(): Promise<OpenProjectOutcome>;
  listRecentProjects(): Promise<readonly RecentProjectSummary[]>;
  openRecentProject(id: string): Promise<OpenProjectOutcome>;
  startupOpenFailure(): Promise<OpenProjectFailure | null>;
}
