import { useEffect } from "react";

import type { Logger } from "../application/logging";
import "./GlobalShell.css";

interface GlobalShellProps {
  logger: Logger;
}

const pendingActions = [
  "Novo Projeto",
  "Abrir Projeto",
  "Exportação em lote",
] as const;

export function GlobalShell({ logger }: GlobalShellProps) {
  useEffect(() => {
    logger.write({
      level: "info",
      component: "global_shell",
      event: "welcome_screen_ready",
    });
  }, [logger]);

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
          {pendingActions.map((action) => (
            <button key={action} type="button" disabled>
              {action}
            </button>
          ))}
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
