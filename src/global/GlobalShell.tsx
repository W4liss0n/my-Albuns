import { useEffect, useState } from "react";

import { logReasonFromError, type Logger } from "../application/logging";
import type { ProjectFileDialog } from "../application/projectFileDialog";
import "./GlobalShell.css";

interface GlobalShellProps {
  logger: Logger;
  projectFileDialog: ProjectFileDialog;
}

export function GlobalShell({ logger, projectFileDialog }: GlobalShellProps) {
  const [dialogStatus, setDialogStatus] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);

  useEffect(() => {
    logger.write({
      level: "info",
      component: "global_shell",
      event: "welcome_screen_ready",
    });
  }, [logger]);

  const openProjectFile = async () => {
    setDialogOpen(true);
    setDialogStatus(null);
    try {
      const selectedPath = await projectFileDialog.openProjectFile();
      if (selectedPath === null) {
        setDialogStatus("Seleção cancelada.");
        logger.write({
          level: "info",
          component: "global_shell",
          event: "project_file_selection_cancelled",
        });
        return;
      }
      setDialogStatus("Arquivo selecionado para validação.");
      logger.write({
        level: "info",
        component: "global_shell",
        event: "project_file_selected",
      });
    } catch (error) {
      setDialogStatus("Não foi possível abrir o seletor de arquivos.");
      logger.write({
        level: "error",
        component: "global_shell",
        event: "project_file_selection_failed",
        reason: logReasonFromError(error),
      });
    } finally {
      setDialogOpen(false);
    }
  };

  return (
    <main className="global-shell">
      <header className="global-shell__header">
        <span className="global-shell__mark" aria-hidden="true">
          M
        </span>
        <strong>MyAlbuns</strong>
      </header>

      <section className="global-shell__workspace">
        <div className="global-shell__recent">
          <p className="global-shell__eyebrow">Tela de Boas-vindas</p>
          <h1>Projetos recentes</h1>
          <div className="global-shell__empty">
            <strong>Nenhum Projeto recente.</strong>
            <span>Abra ou crie um Projeto para começar.</span>
          </div>
        </div>

        <aside className="global-shell__actions" aria-label="Ações globais">
          <button type="button" disabled>
            Novo Projeto
          </button>
          <button type="button" disabled={dialogOpen} onClick={openProjectFile}>
            Abrir Projeto
          </button>
          <button type="button" disabled>
            Exportação em lote
          </button>
          {dialogStatus !== null && (
            <p className="global-shell__dialog-status" role="status">
              {dialogStatus}
            </p>
          )}
        </aside>
      </section>

      <footer className="global-shell__footer">
        <button type="button" disabled>
          Configurações
        </button>
        <span aria-hidden="true">·</span>
        <button type="button" disabled>
          Ajuda
        </button>
      </footer>
    </main>
  );
}
