import type {
  SaveProjectErrorContext,
  SaveProjectFailureCode,
} from "../application/projectPorts";
import type { SaveProjectCommandError as IpcSaveProjectCommandError } from "./generated/SaveProjectCommandError";
import { isIpcRecord, isIpcRevision } from "./ipcGuards";

export interface ProjectSaveFailure {
  code: SaveProjectFailureCode;
  message: string;
  context?: SaveProjectErrorContext;
}

const failureMessages: Readonly<
  Record<IpcSaveProjectCommandError["code"], string>
> = {
  stale_revision:
    "A revisão visível ficou desatualizada. Atualize o Projeto e tente salvar novamente.",
  persisted_baseline_conflict:
    "O arquivo do Projeto foi alterado fora do MyAlbuns. O Salvamento não substituiu essas alterações.",
  save_state_indeterminate:
    "Não foi possível confirmar qual revisão ficou no arquivo. Reabra o Projeto antes de continuar.",
  session_unavailable:
    "A Sessão do Projeto não está mais disponível. Reabra o Projeto para continuar.",
  not_found:
    "O arquivo do Projeto não foi encontrado. Confirme se ele foi movido ou removido.",
  unavailable:
    "O local do Projeto está indisponível. Reconecte a unidade ou o compartilhamento e tente novamente.",
  access_denied:
    "O Windows negou acesso ao arquivo do Projeto. Verifique as permissões e tente novamente.",
  invalid_path: "O caminho do arquivo do Projeto não é válido.",
  unexpected_object_type:
    "O destino do Projeto deixou de ser um arquivo regular.",
  conflict: "O arquivo do Projeto mudou durante o Salvamento. Tente novamente.",
  io_failure: "O Windows não conseguiu concluir o Salvamento do Projeto.",
};

export function parseProjectSaveFailure(
  error: unknown,
): ProjectSaveFailure | null {
  if (!isIpcRecord(error) || typeof error.code !== "string") {
    return null;
  }

  const code = error.code as IpcSaveProjectCommandError["code"];
  if (!(code in failureMessages)) {
    return null;
  }

  if (code === "stale_revision") {
    if (
      !isIpcRevision(error.expectedRevision) ||
      !isIpcRevision(error.currentRevision)
    ) {
      return null;
    }
    return {
      code,
      message: failureMessages[code],
      context: {
        expected: error.expectedRevision,
        current: error.currentRevision,
      },
    };
  }

  return {
    code,
    message: failureMessages[code],
  };
}
