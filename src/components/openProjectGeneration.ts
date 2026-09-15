import type { ProjectGenerationLauncher } from "../application/projectGeneration";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

export async function openProjectGeneration(
  launcher: ProjectGenerationLauncher,
  runner: ProjectMutationRunner,
  setBarrier: (active: boolean) => void,
): Promise<void> {
  setBarrier(true);
  try {
    const pending = await runner.waitForIdle();
    if (pending?.status === "failed") throw pending.error;
    if (pending?.status === "obsolete") return;
    await launcher.open();
  } finally {
    setBarrier(false);
  }
}
