/** Opens a separate Project session without changing the current editor. */
export interface ProjectLauncher {
  newProject(): Promise<void>;
  openProject(): Promise<void>;
}
