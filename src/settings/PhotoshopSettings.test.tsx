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
  const select = screen.getByRole("combobox", { name: "Versão utilizada" });
  await waitFor(() => expect(select).toHaveValue("new"));
  fireEvent.change(select, { target: { value: "old" } });
  await waitFor(() => expect(select).toHaveValue("old"));
  expect(service.select).toHaveBeenCalledExactlyOnceWith("old");
  expect(screen.getByText(installations[1].path)).toBeVisible();
});

test.each(["success", "failure"])("an old focus refresh %s cannot overwrite a newer explicit selection", async (outcome) => {
  const service = port();
  let finish!: (status: PhotoshopStatus) => void;
  let fail!: (error: Error) => void;
  render(<PhotoshopSettings port={service} />);
  await screen.findByText(installations[0].path);
  vi.mocked(service.status).mockImplementationOnce(() => new Promise((resolve, reject) => { finish = resolve; fail = reject; }));
  fireEvent.focus(window);
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "old" } });
  await screen.findByText(installations[1].path);
  await act(async () => { if (outcome === "success") finish(current); else fail(new Error("old read")); });
  expect(screen.getByRole("combobox")).toHaveValue("old");
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("selection owns the pending period and can be retried after failure", async () => {
  const service = port();
  let fail!: (error: Error) => void;
  vi.mocked(service.select).mockImplementationOnce(() => new Promise((_resolve, reject) => { fail = reject; }));
  render(<PhotoshopSettings port={service} />);
  await screen.findByText(installations[0].path);
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "old" } });
  expect(screen.getByRole("combobox")).toBeDisabled();
  const locate = screen.getByRole("button", { name: "Localizar…" });
  expect(locate).toBeDisabled();
  fireEvent.click(locate);
  fireEvent.focus(window);
  expect(service.locate).not.toHaveBeenCalled();
  expect(service.status).toHaveBeenCalledOnce();
  await act(async () => fail(new PhotoshopError("invalid_installation", "Escolha outra instalação.")));
  expect(screen.getByRole("alert")).toHaveTextContent("Escolha outra instalação.");
  expect(screen.getByRole("combobox")).toBeEnabled();
  expect(screen.getByRole("combobox")).toHaveValue("new");
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "old" } });
  await screen.findByText(installations[1].path);
  expect(service.select).toHaveBeenCalledTimes(2);
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("a more recent focus refresh wins even if the earlier read finishes last", async () => {
  const service = port();
  let finish!: (status: PhotoshopStatus) => void;
  render(<PhotoshopSettings port={service} />);
  await screen.findByText(installations[0].path);
  vi.mocked(service.status)
    .mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }))
    .mockResolvedValueOnce({ ...current, revision: 2, selectedInstallationId: "old" });
  fireEvent.focus(window);
  fireEvent.focus(window);
  await screen.findByText(installations[1].path);
  await act(async () => finish(current));
  expect(screen.getByRole("combobox")).toHaveValue("old");
});

test("closing settings stops focus reads while the initial request is still pending", async () => {
  const service = port();
  let fail!: (error: Error) => void;
  vi.mocked(service.status).mockImplementationOnce(() => new Promise((_resolve, reject) => { fail = reject; }));
  const view = render(<PhotoshopSettings port={service} />);
  view.unmount();
  fireEvent.focus(window);
  await act(async () => fail(new Error("late failure")));
  expect(service.status).toHaveBeenCalledOnce();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("manual cancellation preserves selection and an invalid executable displays a recoverable error", async () => {
  const service = port();
  render(<PhotoshopSettings port={service} />);
  await screen.findByText(installations[0].path);
  fireEvent.click(screen.getByRole("button", { name: "Localizar…" }));
  await waitFor(() => expect(screen.getByRole("combobox")).toBeEnabled());
  expect(screen.getByRole("combobox")).toHaveValue("new");
  vi.mocked(service.locate).mockRejectedValueOnce(new PhotoshopError("invalid_installation", "Selecione um executável do Adobe Photoshop."));
  fireEvent.click(screen.getByRole("button", { name: "Localizar…" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Selecione um executável");
  expect(screen.getByRole("combobox")).toHaveValue("new");
});

test("an operation error opens the shared tooltip on the control that failed, without a notice box", async () => {
  const service = port();
  const { container } = render(<PhotoshopSettings port={service} />);
  await screen.findByText(installations[0].path);
  vi.mocked(service.locate).mockRejectedValueOnce(new PhotoshopError("invalid_installation", "Selecione um executável do Adobe Photoshop."));
  const locate = screen.getByRole("button", { name: "Localizar…" });
  fireEvent.click(locate);
  const tooltip = await screen.findByRole("tooltip");
  expect(tooltip).toHaveTextContent("Selecione um executável");
  expect(locate.parentElement).toContainElement(tooltip);
  expect(locate).toHaveAttribute("aria-invalid", "true");
  expect(screen.getByRole("combobox")).not.toHaveAttribute("aria-invalid");
  expect(container.querySelector(".ui-inline-notice")).toBeNull();

  fireEvent.pointerDown(document.body);
  await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());
  expect(screen.getByRole("alert")).toHaveTextContent("Selecione um executável");

  vi.mocked(service.select).mockRejectedValueOnce(new PhotoshopError("invalid_installation", "Escolha outra instalação."));
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "old" } });
  expect(await screen.findByRole("tooltip")).toHaveTextContent("Escolha outra instalação.");
  expect(screen.getByRole("combobox").parentElement).toContainElement(screen.getByRole("tooltip"));
  expect(locate).not.toHaveAttribute("aria-invalid");
});

test("names the command that uses the installation with its shortcut", async () => {
  render(<PhotoshopSettings port={port()} />);
  expect(await screen.findByText("Usado em Abrir no Photoshop (Ctrl+E).")).toBeVisible();
});

test("absence disables only installation selection and focus discovers a later installation", async () => {
  const service = port();
  vi.mocked(service.status).mockResolvedValueOnce({ revision: 0, installations: [], selectedInstallationId: null });
  render(<PhotoshopSettings port={service} />);
  await screen.findByRole("option", { name: "Nenhuma instalação encontrada" });
  expect(screen.getByRole("combobox")).toBeDisabled();
  expect(screen.getByRole("button", { name: "Localizar…" })).toBeEnabled();
  fireEvent.focus(window);
  await waitFor(() => expect(screen.getByRole("combobox")).toHaveValue("new"));
});
