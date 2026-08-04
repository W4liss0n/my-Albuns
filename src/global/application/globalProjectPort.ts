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

export type NewProjectPreset = "neutralV1";

export interface RecentProjectSummary {
  id: string;
  name: string;
}

export interface GlobalProjectPort {
  createProject(preset: NewProjectPreset): Promise<ProjectLaunchOutcome>;
  openProject(): Promise<OpenProjectOutcome>;
  listRecentProjects(): Promise<readonly RecentProjectSummary[]>;
  openRecentProject(id: string): Promise<OpenProjectOutcome>;
  startupOpenFailure(): Promise<OpenProjectFailure | null>;
}
