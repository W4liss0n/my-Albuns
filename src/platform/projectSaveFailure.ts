import type { SaveProjectCommandError as IpcSaveProjectCommandError } from "../contracts/generated/SaveProjectCommandError";
import {
  parseProjectPersistenceFailure,
  type ProjectPersistenceFailure,
} from "./projectPersistenceFailure";

export type ProjectSaveFailure = ProjectPersistenceFailure;

const failureMessages: Readonly<
  Record<IpcSaveProjectCommandError["code"], string>
> = {
  stale_revision:
    "Não foi possível salvar porque há alterações mais recentes no projeto. Nada foi salvo nesta tentativa.",
  persisted_baseline_conflict:
    "O arquivo do projeto foi alterado fora do MyAlbuns. O salvamento não substituiu essas alterações.",
  save_state_indeterminate:
    "Não foi possível confirmar a versão salva no arquivo. Reabra o projeto para conferir o conteúdo antes de continuar.",
  recovery_cleanup_failed:
    "O projeto foi salvo, mas a limpeza dos dados de recuperação não terminou. Tente salvar novamente.",
  session_unavailable:
    "O projeto não está mais disponível para edição. Reabra o projeto para continuar.",
  not_found:
    "O arquivo do projeto não foi encontrado. Confirme se ele foi movido ou removido.",
  unavailable:
    "O local do projeto está indisponível. Reconecte o disco ou a pasta de rede e tente novamente.",
  access_denied:
    "O Windows negou acesso ao arquivo do projeto. Verifique as permissões e tente novamente.",
  invalid_path: "O caminho do arquivo do projeto não é válido.",
  unexpected_object_type:
    "Não é possível salvar: o local do projeto não contém um arquivo válido. Verifique se o arquivo foi movido ou substituído.",
  conflict: "O arquivo do projeto mudou durante o salvamento. Tente novamente.",
  io_failure: "O Windows não conseguiu concluir o salvamento do projeto.",
};

export function parseProjectSaveFailure(
  error: unknown,
): ProjectSaveFailure | null {
  return parseProjectPersistenceFailure(error, failureMessages);
}
