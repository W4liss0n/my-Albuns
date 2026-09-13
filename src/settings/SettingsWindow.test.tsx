import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { CacheSettingsPort } from "../application/cacheSettings";
import { SettingsWindow } from "./SettingsWindow";
import { photoshopSettingsPreview } from "../test/photoshopPreview";

test("Cache cleanup requires confirmation and survives changing tabs while it is pending", async () => {
  let complete!: () => void;
  let clearAllScheduled = false;
  const cachePort: CacheSettingsPort = {
    status: vi.fn(async () => ({ occupiedBytes: 1000, releasableBytes: 500, clearAllScheduled })),
    freeClosedProjects: vi.fn(async () => ({ freedBytes: 500 })),
    clearAll: vi.fn(() => new Promise<{ kind: "scheduled" }>((resolve) => { complete = () => { clearAllScheduled = true; resolve({ kind: "scheduled" }); }; })),
  };
  const close = vi.fn();
  render(<SettingsWindow photoshopPort={photoshopSettingsPreview(null)} cachePort={cachePort} close={close} />);
  const clear = screen.getByRole("button", { name: "Limpar cache" });
  await waitFor(() => expect(clear).toBeEnabled());
  fireEvent.click(clear);
  expect(cachePort.clearAll).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Confirmar" }));
  expect(cachePort.clearAll).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("tab", { name: "Outros" }));
  await act(async () => complete());
  fireEvent.click(screen.getByRole("tab", { name: "Desempenho" }));
  expect(screen.getByRole("status")).toHaveTextContent("Limpeza agendada");
  expect(screen.getByRole("button", { name: "Limpar cache" })).toBeDisabled();
  expect(cachePort.freeClosedProjects).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "Confirmar" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Fechar" }));
  expect(close).toHaveBeenCalledOnce();
});

test("canceling cleanup returns to the cache actions without deleting anything", async () => {
  const cachePort: CacheSettingsPort = {
    status: async () => ({ occupiedBytes: 1000, releasableBytes: 500, clearAllScheduled: false }),
    freeClosedProjects: vi.fn(async () => ({ freedBytes: 500 })),
    clearAll: vi.fn(async () => ({ kind: "scheduled" as const })),
  };
  render(<SettingsWindow photoshopPort={photoshopSettingsPreview(null)} cachePort={cachePort} close={vi.fn()} />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Limpar cache" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Limpar cache" }));
  expect(screen.getByRole("button", { name: "Cancelar" })).toHaveFocus();
  fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(screen.getByRole("button", { name: "Limpar cache" })).toHaveFocus();
  expect(cachePort.freeClosedProjects).not.toHaveBeenCalled();
  expect(cachePort.clearAll).not.toHaveBeenCalled();
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
  expect(screen.getByRole("tab", { name: "Outros" })).toHaveAttribute("aria-selected", "true");
  view.unmount();
  expect(release).toHaveBeenCalledOnce();
});
