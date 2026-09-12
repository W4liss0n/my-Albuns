import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { PhotoshopSettingsPort, PhotoshopStatus } from "../application/photoshop";
import { PhotoshopError } from "../application/photoshop";
import { PhotoshopSettings } from "./PhotoshopSettings";

const installations = [
  { id: "new", name: "Adobe Photoshop 2026", version: "27.10", path: "C:\\Adobe\\2026\\Photoshop.exe" },
  { id: "old", name: "Adobe Photoshop 2025", version: "26.8", path: "C:\\Adobe\\2025\\Photoshop.exe" },
];
const current: PhotoshopStatus = { revision: 1, installations, selectedInstallationId: "new" };
const port = (): PhotoshopSettingsPort => ({ status: vi.fn(async () => current), select: vi.fn(async (id) => ({ ...current, revision: 2, selectedInstallationId: id })), locate: vi.fn(async () => null) });

test("loads the persisted installation and commits an explicit alternative", async () => {
  const service = port();
  render(<PhotoshopSettings port={service} />);
  const select = screen.getByRole("combobox", { name: "Instalação usada" });
  await waitFor(() => expect(select).toHaveValue("new"));
  fireEvent.change(select, { target: { value: "old" } });
  await waitFor(() => expect(select).toHaveValue("old"));
  expect(service.select).toHaveBeenCalledExactlyOnceWith("old");
  expect(screen.getByText(installations[1].path)).toBeVisible();
});

test("an old focus refresh cannot overwrite a newer explicit selection", async () => {
  const service = port();
  let finish!: (status: PhotoshopStatus) => void;
  render(<PhotoshopSettings port={service} />);
  await screen.findByText(installations[0].path);
  vi.mocked(service.status).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  fireEvent.focus(window);
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "old" } });
  await screen.findByText(installations[1].path);
  await act(async () => finish(current));
  expect(screen.getByRole("combobox")).toHaveValue("old");
});

test("manual cancellation preserves selection and an invalid executable displays a recoverable error", async () => {
  const service = port();
  render(<PhotoshopSettings port={service} />);
  await screen.findByText(installations[0].path);
  fireEvent.click(screen.getByRole("button", { name: "Localizar Photoshop…" }));
  await waitFor(() => expect(screen.getByRole("combobox")).toBeEnabled());
  expect(screen.getByRole("combobox")).toHaveValue("new");
  vi.mocked(service.locate).mockRejectedValueOnce(new PhotoshopError("invalid_installation", "Selecione um executável do Adobe Photoshop."));
  fireEvent.click(screen.getByRole("button", { name: "Localizar Photoshop…" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Selecione um executável");
  expect(screen.getByRole("combobox")).toHaveValue("new");
});

test("absence disables only installation selection and focus discovers a later installation", async () => {
  const service = port();
  vi.mocked(service.status).mockResolvedValueOnce({ revision: 0, installations: [], selectedInstallationId: null });
  render(<PhotoshopSettings port={service} />);
  await screen.findByText(/Photoshop não encontrado/);
  expect(screen.getByRole("combobox")).toBeDisabled();
  expect(screen.getByRole("button", { name: "Localizar Photoshop…" })).toBeEnabled();
  fireEvent.focus(window);
  await waitFor(() => expect(screen.getByRole("combobox")).toHaveValue("new"));
});
