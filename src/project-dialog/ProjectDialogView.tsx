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
import { ExportConfigurationDialog } from "./ExportConfigurationDialog";
import { StorageFullDialog } from "../ui/StorageFullDialog";

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
    case "storageFull": return <StorageFullDialog state={state} onAction={onAction} />;
    case "exportConfiguration": return <ExportConfigurationDialog state={state} onAction={onAction} />;
    case "exportConflicts": return <ConfirmationDialog title="Já existe uma exportação"
      description="Há arquivos exportados na pasta de destino. Deseja ignorar os existentes ou substituí-los?"
      leadingAction={{ label: "Ignorar", onClick: () => onAction("skipExportConflicts") }}
      cancelAction={{ label: "Cancelar", onClick: () => onAction("dismissExport") }}
      confirmAction={{ label: "Substituir", onClick: () => onAction("confirmExportOverwrite") }} />;
    case "mediaRemovalConfirmation":
      return <ConfirmationDialog title={`Remover ${state.count} ${state.mediaKind === "photo" ? (state.count === 1 ? "Foto" : "Fotos") : (state.count === 1 ? "Decorativo" : "Decorativos")}?`} tone="danger"
        description={state.mediaKind === "photo"
          ? `${state.usedCount} da seleção em uso, em ${state.usageCount} Quadros. Remover tudo exclui quadros destravados e mantém as posições travadas vazias.`
          : `${state.usedCount} da seleção em uso. As aplicações removidas voltam ao padrão do álbum; padrões removidos passam a fundo branco ou sobreposição ausente.`}
        cancelAction={{ label: "Cancelar", disabled: state.busy, onClick: () => onAction("cancelMediaRemoval") }}
        leadingAction={state.mediaKind === "photo" ? { label: "Remover imagens e manter quadros", disabled: state.busy, onClick: () => onAction("removeMediaKeepFrames") } : undefined}
        confirmAction={{ label: state.busy ? "Removendo…" : state.mediaKind === "photo" ? "Remover tudo" : "Remover", disabled: state.busy, onClick: () => onAction("removeAllMedia") }} />;
    case "layoutDeletionConfirmation":
      return <ConfirmationDialog title="Excluir layout personalizado?" tone="danger"
        description="Este layout será removido da lista de personalizados. Os álbuns que já o usam e as cópias salvas nos projetos serão mantidos."
        cancelAction={{ label: "Cancelar", disabled: state.busy, onClick: () => onAction("cancelLayoutDeletion") }}
        confirmAction={{ label: state.busy ? "Excluindo…" : "Excluir", disabled: state.busy, onClick: () => onAction("confirmLayoutDeletion") }} />;
    case "edgeConversionConfirmation":
      return <ConfirmationDialog title="Converter para página única?" tone="danger"
        description={state.message}
        cancelAction={{ label: "Cancelar", onClick: () => onAction("cancelEdgeConversion") }}
        confirmAction={{ label: "Converter", onClick: () => onAction("confirmEdgeConversion") }} />;
    case "exportMediaProblems":
      return <ProblemsDialog title="Problemas na exportação"
        description={state.message || "Resolva os problemas abaixo para exportar."}
        columns={["Projeto", "Problema", "Ações"]}
        rows={state.problems.map(problem => [state.projectName,
          `${problem.fileName}: arquivo ${problem.state === "absent" ? "ausente" : "indisponível"}.`,
          <ActionButton disabled={state.busy} onClick={() => onAction(problem.state === "absent" ? "relinkExportMedia" : "retryExportMedia")}>
            {problem.state === "absent" ? "Localizar imagens…" : "Tentar novamente"}
          </ActionButton>])}
        closeDisabled={state.busy}
        onClose={() => onAction("dismissExport")} />;
    case "exportProblems":
      return <ProblemsDialog title="Problemas na exportação"
        description="Adicione fotos aos quadros vazios para exportar a seleção."
        columns={["Projeto", "Motivo", "Ação"]}
        rows={state.problems.map((problem) => [state.projectName,
          `Lâmina ${String(problem.sheetNumber).padStart(2, "0")}, posição ${problem.frameNumber}: quadro vazio.`,
          <ActionButton onClick={() => onAction("openExportProject")}>Voltar ao álbum</ActionButton>])}
        onClose={() => onAction("dismissExport")} />;
    case "imageProcessingProgress":
      return <ProgressDialog title="Processando imagens" progress={state.progress} />;
    case "imageProcessingProblems": {
      const imported = state.importedCount === null ? "" : state.importedCount === 0 ? "Nenhuma imagem nova foi importada." :
        state.importedCount === 1 ? "1 imagem importada." : `${state.importedCount} imagens importadas.`;
      const title = state.operationProblem ? state.importedCount === null ? "Processamento interrompido" : "Importação interrompida" : "Problemas no processamento";
      const description = `${imported} ${state.operationProblem ?? "Confira os arquivos que não puderam ser processados por completo."}`.trim();
      if (state.operationProblem && state.problems.length === 0) {
        return <MessageDialog title={title} tone="error" description={description}
          secondaryAction={{ label: "Fechar", onClick: () => onAction("dismissImageProcessingProblems") }} />;
      }
      return <ProblemsDialog title={title} description={description}
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
          title="Aplicar alterações no álbum?"
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
          description="O projeto tem alterações que ainda não foram salvas."
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
          title="Não foi possível fechar o projeto"
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
                O editor precisa de aceleração gráfica para continuar.
              </p>
            </>
          }
          secondaryAction={{
            label: "Fechar projeto",
            onClick: () => onAction("closeProjectAfterGraphicsFailure"),
          }}
          secondaryButtonRef={graphicsFailureActionRef}
          title="Não foi possível iniciar o editor"
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
                    : "Cancelar exportação",
                  onClick: () => onAction("cancelExport"),
                }
              : undefined
          }
          progress={state.progress.kind === "determinate"
            ? { ...state.progress, status: null, countLabel: state.progress.status }
            : { ...state.progress, status: null }}
          title="Exportando"
          reserveProgressMeta
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
