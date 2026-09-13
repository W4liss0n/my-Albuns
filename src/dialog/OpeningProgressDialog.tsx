import type { StartupImageProgress } from "../platform/generated/StartupImageProgress";
import { ProgressDialog } from "../ui";

export function OpeningProgressDialog({
  creating = false,
  images = null,
}: {
  creating?: boolean;
  images?: StartupImageProgress | null;
}) {
  return <ProgressDialog
    title={creating ? "Criando Projeto" : "Abrindo Projeto"}
    reserveProgressMeta
    progress={images ? {
      kind: "determinate",
      completed: images.completedFiles,
      total: images.totalFiles,
      status: "Preparando imagens",
      remaining: `${images.completedFiles} de ${images.totalFiles}`,
    } : {
      kind: "indeterminate",
      status: "Preparando a Janela do Projeto…",
    }}
  />;
}
