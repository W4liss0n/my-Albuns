import type {
  ProjectDialogAction,
  ProjectDialogState,
} from "../../application/projectDialogPort";

export interface ProjectDialogClient {
  onState(
    listener: (state: ProjectDialogState) => void,
  ): Promise<() => void>;
  submit(action: ProjectDialogAction): Promise<void>;
}
