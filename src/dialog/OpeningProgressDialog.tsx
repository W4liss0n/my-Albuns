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
    title={creating ? "Criando projeto" : "Abrindo projeto"}
    reserveProgressMeta
    progress={images ? {
      kind: "determinate",
      completed: images.completedFiles,
      total: images.totalFiles,
      status: "Preparando imagens",
    } : {
      kind: "indeterminate",
      status: "Preparando a Janela do projeto…",
    }}
  />;
}
