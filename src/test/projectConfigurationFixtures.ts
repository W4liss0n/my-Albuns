import type { ProjectConfigurationRasterLimits } from "../domain/generated/ProjectConfigurationRasterLimits";

// Facts from the Core validation for the neutral 300 DPI project.
export const rasterLimitsAt300Dpi: ProjectConfigurationRasterLimits = {
  sheetWidth: { minimumUm: 86, maximumUm: 5_548_672 },
  sheetHeight: { minimumUm: 43, maximumUm: 5_548_672 },
};
