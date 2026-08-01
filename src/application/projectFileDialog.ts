export interface ProjectFileDialog {
  openProjectFile(): Promise<string | null>;
}
