import type {
  ProjectDialogAction,
  ProjectDialogPresentation,
} from "../../application/projectDialogPort";

export interface ProjectDialogClient {
  onPresentation(
    listener: (presentation: ProjectDialogPresentation) => void,
  ): Promise<() => void>;
  submit(sessionId: string, action: ProjectDialogAction): Promise<void>;
}
