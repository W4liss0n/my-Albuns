import { ProgressDialog } from "../ui";

interface OpeningImageProgress {
  completedFiles: number;
  totalFiles: number;
}

export function OpeningProgressDialog({
  creating = false,
  images = null,
}: {
  creating?: boolean;
  images?: OpeningImageProgress | null;
}) {
  return <ProgressDialog
    title={creating ? "Criando Projeto" : "Abrindo Projeto"}
    reserveProgressMeta
    progress={images ? {
      kind: "determinate",
      completed: images.completedFiles,
      total: images.totalFiles,
      status: "Preparando imagens",
    } : {
      kind: "indeterminate",
      status: "Preparando a Janela do Projeto…",
    }}
  />;
}
