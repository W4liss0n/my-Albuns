import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { CacheSettingsPort } from "../application/cacheSettings";
import { SettingsWindow } from "./SettingsWindow";
import { photoshopSettingsPreview } from "../test/photoshopPreview";

test("Cache cleanup requires confirmation and survives changing tabs while it is pending", async () => {
  let complete!: () => void;
  const cachePort: CacheSettingsPort = {
    status: vi.fn(async () => ({ occupiedBytes: 1000, releasableBytes: 500, clearAllScheduled: false })),
    freeClosedProjects: vi.fn(async () => ({ freedBytes: 500 })),
    clearAll: vi.fn(() => new Promise((resolve) => { complete = () => resolve({ kind: "scheduled" }); })),
  };
  const close = vi.fn();
  render(<SettingsWindow photoshopPort={photoshopSettingsPreview(null)} cachePort={cachePort} close={close} />);
  const clear = screen.getByRole("button", { name: "Limpar todo o Cache" });
  await waitFor(() => expect(clear).toBeEnabled());
  fireEvent.click(clear);
  expect(cachePort.clearAll).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Confirmar" }));
  expect(cachePort.clearAll).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("tab", { name: "Photoshop" }));
  await act(async () => complete());
  fireEvent.click(screen.getByRole("tab", { name: "Desempenho" }));
  expect(screen.getByRole("status")).toHaveTextContent("Limpeza agendada");
  expect(screen.queryByRole("button", { name: "Confirmar" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Fechar", exact: true }));
  expect(close).toHaveBeenCalledOnce();
});

test("a forwarded settings request changes section and unregisters its listener on close", async () => {
  let request!: (section: "photoshop" | "performance") => void;
  const release = vi.fn();
  const cachePort: CacheSettingsPort = {
    status: async () => ({ occupiedBytes: 0, releasableBytes: 0, clearAllScheduled: false }),
    freeClosedProjects: async () => ({ freedBytes: 0 }), clearAll: async () => ({ kind: "scheduled" }),
  };
  const view = render(<SettingsWindow photoshopPort={photoshopSettingsPreview(null)} cachePort={cachePort} close={vi.fn()}
    onSectionRequest={async (listener) => { request = listener; return release; }} />);
  await act(async () => request("photoshop"));
  expect(screen.getByRole("tab", { name: "Photoshop" })).toHaveAttribute("aria-selected", "true");
  view.unmount();
  expect(release).toHaveBeenCalledOnce();
});
