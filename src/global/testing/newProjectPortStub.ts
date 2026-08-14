import type { NewProjectPort } from "../application/globalProjectPort";

/** PREVIEW/TEST STUB: this is not a production NewProjectPort implementation. */
export function createNewProjectPortStub(
  overrides: Partial<NewProjectPort> = {},
): NewProjectPort {
  return {
    chooseProvisionalDecorative: async () => ({ status: "cancelled" }),
    createProject: async () => ({ status: "cancelled" }),
    releaseProvisionalDecorative: async () => undefined,
    validateProjectConfiguration: async () => ({ status: "valid" }),
    ...overrides,
  };
}
