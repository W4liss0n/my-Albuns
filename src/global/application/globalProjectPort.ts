export interface OpenProjectFailure {
  code: string;
  stage?: string;
  message: string;
  action?: string;
}

export type OpenProjectOutcome =
  | { status: "opened" }
  | { status: "cancelled" }
  | { status: "failed"; error: OpenProjectFailure };

export interface RecentProjectSummary {
  id: string;
  name: string;
}

export interface GlobalProjectPort {
  openProject(): Promise<OpenProjectOutcome>;
  listRecentProjects(): Promise<readonly RecentProjectSummary[]>;
  openRecentProject(id: string): Promise<OpenProjectOutcome>;
  startupOpenFailure(): Promise<OpenProjectFailure | null>;
}
