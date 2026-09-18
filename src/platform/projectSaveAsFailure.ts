import type { SaveAsProjectCommandError as IpcSaveAsProjectCommandError } from "../contracts/generated/SaveAsProjectCommandError";
import { SAVE_AS_STATE_INDETERMINATE_MESSAGE } from "../application/projectSaveAsStartup";
import {
  parseProjectPersistenceFailure,
  type ProjectPersistenceFailure,
} from "./projectPersistenceFailure";

export type ProjectSaveAsFailure = ProjectPersistenceFailure;

const failureMessages: Readonly<
  Record<IpcSaveAsProjectCommandError["code"], string>
> = {
  stale_revision:
    "Não foi possível salvar a cópia porque há alterações mais recentes no projeto. Nada foi salvo nesta tentativa.",
  same_target:
    "Escolha outro arquivo: Salvar como não pode usar o próprio projeto atual como destino.",
  destination_conflict:
    "O destino mudou depois da confirmação. Nenhum arquivo foi substituído.",
  project_in_use:
    "O projeto escolhido está aberto em outra janela. Escolha outro arquivo para salvar a cópia.",
  identity_indeterminate:
    "Não foi possível verificar o arquivo do projeto ou o destino escolhido. Confira se o arquivo original continua no mesmo local antes de tentar novamente.",
  save_as_state_indeterminate:
    SAVE_AS_STATE_INDETERMINATE_MESSAGE,
  session_unavailable:
    "O projeto não está mais disponível para edição. Reabra o projeto para continuar.",
  dialog_unavailable:
    "Não foi possível abrir a janela para salvar o projeto.",
  not_found: "O local escolhido para Salvar como não foi encontrado.",
  unavailable:
    "O destino está indisponível. Reconecte o disco ou a pasta de rede e tente novamente.",
  access_denied:
    "O Windows negou acesso ao destino de Salvar como. Verifique as permissões e tente novamente.",
  invalid_path: "O caminho escolhido para Salvar como não é válido.",
  unexpected_object_type:
    "O destino escolhido não é um arquivo válido. Escolha outro arquivo para salvar a cópia.",
  conflict: "O destino mudou durante Salvar como. Tente novamente.",
  io_failure: "O Windows não conseguiu concluir Salvar como.",
};

export function parseProjectSaveAsFailure(
  error: unknown,
): ProjectSaveAsFailure | null {
  return parseProjectPersistenceFailure(error, failureMessages);
}
