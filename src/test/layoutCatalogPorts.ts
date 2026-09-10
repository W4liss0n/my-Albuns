import type { ProjectCorePort } from "../application/projectPorts";
import type { ProjectDialogPort } from "../application/projectDialogPort";

export const emptyLayoutCatalogPort: Pick<ProjectCorePort, "refreshLayoutCatalog" | "saveCustomLayout" | "deleteCustomLayout"> = {
  refreshLayoutCatalog: async () => 0,
  saveCustomLayout: async () => { throw new Error("This fixture has no custom Layout write adapter"); },
  deleteCustomLayout: async () => { throw new Error("This fixture has no custom Layout write adapter"); },
};

export const unusedLayoutDialogPort: ProjectDialogPort = {
  acquire: () => { throw new Error("This fixture does not open a Layout deletion dialog"); },
};
