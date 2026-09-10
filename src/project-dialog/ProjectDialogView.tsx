import { useLayoutEffect, useRef } from "react";

import type {
  ProjectDialogAction,
  ProjectDialogState,
} from "../application/projectDialogPort";
import {
  ActionButton,
  ConfirmationDialog,
  MessageDialog,
  ProgressDialog,
  ProblemsDialog,
} from "../ui";
import "./ProjectDialogView.css";

interface ProjectDialogViewProps {
  onAction(action: ProjectDialogAction): void;
  state: ProjectDialogState;
}

export function ProjectDialogView({
  onAction,
  state,
}: ProjectDialogViewProps) {
  const graphicsFailureActionRef = useRef<HTMLButtonElement>(null);

  useLayoutEffect(() => {
    if (state.kind === "graphicsFailure") {
      graphicsFailureActionRef.current?.focus({ preventScroll: true });
    }
  }, [state.kind]);

  switch (state.kind) {
    case "layoutDeletionConfirmation":
      return <ConfirmationDialog title="Excluir Layout personalizado?" tone="danger"
        description="O Layout será removido do catálogo em todas as Janelas. As composições aplicadas e as cópias guardadas nos Projetos serão preservadas."
        cancelAction={{ label: "Cancelar", disabled: state.busy, onClick: () => onAction("cancelLayoutDeletion") }}
        confirmAction={{ label: state.busy ? "Excluindo…" : "Excluir", disabled: state.busy, onClick: () => onAction("confirmLayoutDeletion") }} />;
    case "exportProblems":
      return <ProblemsDialog title="Problemas na Exportação"
        description="Preencha os Frames vazios para exportar a seleção."
        columns={["Projeto", "Motivo", "Ação"]}
        rows={state.problems.map((problem) => [state.projectName,
          `Lâmina ${String(problem.sheetNumber).padStart(2, "0")}, posição ${problem.frameNumber}: Frame vazio.`,
          <ActionButton onClick={() => onAction("openExportProject")}>Abrir Projeto</ActionButton>])}
        onClose={() => onAction("dismissExport")} />;
    case "imageProcessingProgress":
      return <ProgressDialog title="Processando Imagens" progress={state.progress} />;
    case "imageProcessingProblems": {
      const imported = state.importedCount === null ? "" : state.importedCount === 0 ? "Nenhuma Foto nova foi importada." :
        state.importedCount === 1 ? "1 Foto importada." : `${state.importedCount} Fotos importadas.`;
      return <ProblemsDialog title="Problemas no processamento" description={`${imported} Confira os arquivos que não puderam ser processados por completo.`.trim()}
        columns={["Arquivo", "Motivo"]}
        rows={state.problems.map(problem => [problem.fileName, problem.reason])}
        onClose={() => onAction("dismissImageProcessingProblems")} />;
    }
    case "albumInformationConfirmation":
      return (
        <ConfirmationDialog
          cancelAction={{
            disabled: state.busy,
            label: "Cancelar",
            onClick: () => onAction("cancelAlbumInformation"),
          }}
          confirmAction={{
            disabled: state.busy,
            label: state.busy ? "Aplicando…" : "Aplicar",
            onClick: () => onAction("confirmAlbumInformation"),
          }}
          description="As alterações serão aplicadas juntas e poderão ser desfeitas em uma única ação."
          title="Aplicar alterações no Álbum?"
        >
          <dl className="album-information-change-list">
            {state.details.map((detail) => {
              return (
                <div
                  className="album-information-change"
                  key={`${detail.label}:${detail.value}`}
                >
                  <dt className="album-information-change__label">
                    {detail.label}
                  </dt>
                  <dd className="album-information-change__value">
                    {detail.value}
                  </dd>
                </div>
              );
            })}
          </dl>
        </ConfirmationDialog>
      );

    case "projectCloseConfirmation":
      return (
        <ConfirmationDialog
          cancelAction={{
            disabled: state.busy,
            label: "Cancelar",
            onClick: () => onAction("cancelProjectClose"),
          }}
          confirmAction={{
            disabled: state.busy,
            label: state.busy ? "Salvando…" : "Salvar e fechar",
            onClick: () => onAction("saveAndClose"),
          }}
          description="O Projeto tem alterações que ainda não foram salvas."
          leadingAction={{
            disabled: state.busy,
            label: "Descartar e fechar",
            onClick: () => onAction("discardAndClose"),
          }}
          title="Salvar alterações antes de fechar?"
        />
      );

    case "projectCloseFailure":
      return (
        <MessageDialog
          description={state.message}
          secondaryAction={{
            label: "Fechar",
            onClick: () => onAction("dismissProjectCloseFailure"),
          }}
          title="Não foi possível fechar o Projeto"
          tone="error"
        />
      );

    case "projectOperationFailure":
      return (
        <MessageDialog
          description={state.message}
          secondaryAction={{
            label: "Fechar",
            onClick: () => onAction("dismissProjectOperationFailure"),
          }}
          title="A operação não foi concluída"
          tone="error"
        />
      );

    case "graphicsFailure":
      return (
        <MessageDialog
          description={
            <>
              <p>{state.reason}</p>
              <p>
                O editor não continuará sem WebGL2 acelerado por hardware.
              </p>
            </>
          }
          secondaryAction={{
            label: "Fechar Projeto",
            onClick: () => onAction("closeProjectAfterGraphicsFailure"),
          }}
          secondaryButtonRef={graphicsFailureActionRef}
          title="O Canvas não pôde ser iniciado"
          tone="error"
        />
      );

    case "exportProgress":
      return (
        <ProgressDialog
          cancelAction={
            state.cancellable
              ? {
                  disabled: state.cancelRequested,
                  label: state.cancelRequested
                    ? "Cancelando…"
                    : "Cancelar Exportação",
                  onClick: () => onAction("cancelExport"),
                }
              : undefined
          }
          progress={state.progress}
          title="Exportando"
        />
      );

    case "exportFailure":
      return (
        <MessageDialog
          description={state.message}
          primaryAction={{
            disabled: state.retryDisabled,
            label: "Tentar novamente",
            onClick: () => onAction("retryExport"),
          }}
          secondaryAction={{
            label: "Fechar",
            onClick: () => onAction("dismissExport"),
          }}
          title={
            state.cancelled
              ? "Exportação cancelada"
              : "Exportação não concluída"
          }
          tone="error"
        />
      );

    case "exportSuccess":
      return (
        <MessageDialog
          description={state.message}
          secondaryAction={{
            label: "Fechar",
            onClick: () => onAction("dismissExport"),
          }}
          title="Exportação concluída"
          tone="success"
        />
      );
  }
}
