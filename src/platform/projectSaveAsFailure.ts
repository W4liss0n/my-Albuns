import type { SaveAsProjectCommandError as IpcSaveAsProjectCommandError } from "./generated/SaveAsProjectCommandError";
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
    "A revisão visível ficou desatualizada. Atualize o Projeto e tente Salvar como novamente.",
  same_target:
    "Escolha outro arquivo: Salvar como não pode usar o próprio Projeto atual como destino.",
  destination_conflict:
    "O destino mudou depois da confirmação. Nenhum arquivo foi substituído.",
  project_in_use:
    "O Projeto escolhido como destino está aberto para edição em outra Sessão.",
  identity_indeterminate:
    "Não foi possível comprovar a Identidade física do Projeto ou do destino.",
  save_as_state_indeterminate:
    SAVE_AS_STATE_INDETERMINATE_MESSAGE,
  session_unavailable:
    "A Sessão do Projeto não está mais disponível. Reabra o Projeto para continuar.",
  dialog_unavailable:
    "O diálogo nativo de Salvar como não pôde ser aberto.",
  not_found: "O local escolhido para Salvar como não foi encontrado.",
  unavailable:
    "O destino está indisponível. Reconecte a unidade ou o compartilhamento e tente novamente.",
  access_denied:
    "O Windows negou acesso ao destino de Salvar como. Verifique as permissões e tente novamente.",
  invalid_path: "O caminho escolhido para Salvar como não é válido.",
  unexpected_object_type:
    "O destino de Salvar como deixou de ser um arquivo regular.",
  conflict: "O destino mudou durante Salvar como. Tente novamente.",
  io_failure: "O Windows não conseguiu concluir Salvar como.",
};

export function parseProjectSaveAsFailure(
  error: unknown,
): ProjectSaveAsFailure | null {
  return parseProjectPersistenceFailure(error, failureMessages);
}
