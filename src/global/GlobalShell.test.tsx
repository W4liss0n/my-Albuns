import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

import type { Logger } from "../application/logging";
import { GlobalShell } from "./GlobalShell";

describe("GlobalShell", () => {
  test("renders the Welcome Screen and reports readiness", async () => {
    const write = vi.fn<Logger["write"]>();
    const openProjectFile = vi.fn();

    render(
      <GlobalShell
        logger={{ write }}
        projectFileDialog={{ openProjectFile }}
      />,
    );

    expect(
      screen.getByRole("heading", { name: "Projetos recentes" }),
    ).toBeInTheDocument();
    for (const action of [
      "Novo Projeto",
      "Exportação em lote",
      "Configurações",
      "Ajuda",
    ]) {
      expect(screen.getByRole("button", { name: action })).toBeDisabled();
    }
    await waitFor(() => {
      expect(write).toHaveBeenCalledWith({
        level: "info",
        component: "global_shell",
        event: "welcome_screen_ready",
      });
    });
  });

  test("reports a file selected through the native dialog", async () => {
    const user = userEvent.setup();
    const write = vi.fn<Logger["write"]>();
    const openProjectFile = vi
      .fn()
      .mockResolvedValue(String.raw`C:\Álbuns\Casamento.myalbuns`);

    render(
      <GlobalShell
        logger={{ write }}
        projectFileDialog={{ openProjectFile }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));

    expect(openProjectFile).toHaveBeenCalledOnce();
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Arquivo selecionado para validação.",
    );
    expect(write).toHaveBeenCalledWith({
      level: "info",
      component: "global_shell",
      event: "project_file_selected",
    });
  });

  test("keeps the Welcome Screen ready when the native dialog is cancelled", async () => {
    const user = userEvent.setup();
    const write = vi.fn<Logger["write"]>();
    const openProjectFile = vi.fn().mockResolvedValue(null);

    render(
      <GlobalShell
        logger={{ write }}
        projectFileDialog={{ openProjectFile }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));

    expect(await screen.findByRole("status")).toHaveTextContent(
      "Seleção cancelada.",
    );
    expect(write).toHaveBeenCalledWith({
      level: "info",
      component: "global_shell",
      event: "project_file_selection_cancelled",
    });
  });
});
