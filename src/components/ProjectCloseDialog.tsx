import { Dialog, Modal, ModalOverlay } from "react-aria-components";

import type { ProjectCloseChoice } from "../application/projectPorts";

interface ProjectCloseDialogProps {
  busy: boolean;
  onChoose(choice: ProjectCloseChoice): void;
}

export function ProjectCloseDialog({
  busy,
  onChoose,
}: ProjectCloseDialogProps) {
  return (
    <ModalOverlay
      className="project-close-backdrop"
      isKeyboardDismissDisabled
      isOpen
    >
      <Modal className="project-close-modal">
        <Dialog
          aria-labelledby="project-close-title"
          className="project-close-dialog"
        >
          <h2 id="project-close-title">Salvar alterações antes de fechar?</h2>
          <p>
            O Projeto tem alterações que ainda não foram gravadas no arquivo.
          </p>
          <div className="project-close-actions">
            <button
              className="project-close-primary"
              disabled={busy}
              type="button"
              onClick={() => onChoose("saveAndClose")}
            >
              Salvar e fechar
            </button>
            <button
              disabled={busy}
              type="button"
              onClick={() => onChoose("discardAndClose")}
            >
              Descartar e fechar
            </button>
            <button
              disabled={busy}
              type="button"
              onClick={() => onChoose("cancel")}
            >
              Cancelar
            </button>
          </div>
          {busy && <p aria-live="polite">Concluindo…</p>}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
