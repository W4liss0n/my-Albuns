import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState, type ComponentProps } from "react";
import { expect, test, vi } from "vitest";

import type { ProjectedVisualDefaults } from "../domain/project";
import { representativeProjection } from "../test/projectFixtures";
import { AlbumDesignForm } from "./AlbumDesignForm";

function cloneVisualDefaults(
  value: ProjectedVisualDefaults,
): ProjectedVisualDefaults {
  return {
    ...value,
    background: { ...value.background },
    overlay: { ...value.overlay },
    frameBorder: { ...value.frameBorder },
  };
}

function Harness({
  onApply,
  revision = representativeProjection.state.revision,
  frameGapUm = 5_000,
  value,
}: {
  onApply: ComponentProps<typeof AlbumDesignForm>["onApply"];
  revision?: number;
  frameGapUm?: number;
  value: ProjectedVisualDefaults;
}) {
  const [ready, setReady] = useState(false);
  return (
    <>
      <AlbumDesignForm
        frameGapUm={frameGapUm}
        document={representativeProjection.state.document}
        formId="album-design-equivalent-projection"
        mediaItems={[]}
        mediaPreviews={{}}
        presentationUnit="mm"
        revision={revision}
        value={value}
        onApply={onApply}
        onReadyChange={setReady}
      />
      <button
        disabled={!ready}
        form="album-design-equivalent-projection"
        type="submit"
      >
        Aplicar
      </button>
    </>
  );
}

test("retains existing border and gap values above the standard creation range", async () => {
  const value: ProjectedVisualDefaults = {
    ...representativeProjection.state.album.visualDefaults,
    frameBorder: { kind: "solid", rgb: "#123456", widthUm: 8_000 },
  };
  const onApply = vi.fn<ComponentProps<typeof AlbumDesignForm>["onApply"]>(async () => true);
  render(<Harness onApply={onApply} value={value} frameGapUm={30_000} />);
  const border = screen.getByRole("slider", { name: "Espessura da borda" });
  const gap = screen.getByRole("slider", { name: "Espaço entre quadros" });
  expect(border).toHaveValue("8000");
  expect(border).toHaveAttribute("max", "8000");
  expect(gap).toHaveValue("30000");
  expect(gap).toHaveAttribute("max", "30000");
  fireEvent.change(border, { target: { value: "0" } });
  expect(screen.getByText("sem borda")).toBeVisible();
  expect(border).toHaveAttribute("max", "8000");
  fireEvent.change(border, { target: { value: "7750" } });
  fireEvent.change(gap, { target: { value: "29000" } });
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());
  expect(onApply.mock.calls[0][0].materialize(representativeProjection)).toMatchObject({
    visualDefaults: { frameBorder: { kind: "solid", rgb: "#123456", widthUm: 7_750 } },
    frameGapUm: 29_000,
  });
});

test("preserves an unapplied draft across a semantically equivalent projection", async () => {
  const onApply = vi.fn<ComponentProps<typeof AlbumDesignForm>["onApply"]>();
  const baseline = representativeProjection.state.album.visualDefaults;
  const view = render(<Harness onApply={onApply} value={baseline} />);
  fireEvent.change(screen.getByLabelText("Cor do fundo"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );

  view.rerender(
    <Harness onApply={onApply} value={cloneVisualDefaults(baseline)} />,
  );

  expect(screen.getByLabelText("Cor do fundo")).toHaveValue("#f7f5f0");
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled();
});

test("edits the confirmed Project gap as part of Apply and adopts the applied baseline", async () => {
  const value = representativeProjection.state.album.visualDefaults;
  const onApply = vi.fn<ComponentProps<typeof AlbumDesignForm>["onApply"]>(async () => true);
  const view = render(<Harness onApply={onApply} value={value} frameGapUm={7_000} />);
  expect(screen.getByRole("slider", { name: "Espaço entre quadros" })).toHaveValue("7000");
  fireEvent.change(screen.getByRole("slider", { name: "Espaço entre quadros" }), { target: { value: "9000" } });
  expect(onApply).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());
  const submitted = onApply.mock.calls[0][0];
  expect(submitted.materialize(representativeProjection)).toEqual({
    kind: "setAlbumDesign", frameGapUm: 9_000, visualDefaults: value,
  });
  view.rerender(<Harness onApply={onApply} value={value} frameGapUm={9_000} revision={26} />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Aplicar" })).toBeDisabled());
  expect(screen.getByRole("slider", { name: "Espaço entre quadros" })).toHaveValue("9000");
});

test("rebases an unapplied draft over an authoritative Album design change", async () => {
  const onApply = vi.fn<ComponentProps<typeof AlbumDesignForm>["onApply"]>();
  const baseline = representativeProjection.state.album.visualDefaults;
  const changed: ProjectedVisualDefaults = {
    ...cloneVisualDefaults(baseline),
    background: {
      scope: "bothSides",
      both: { kind: "color", rgb: "#112233" },
    },
  };
  const view = render(<Harness onApply={onApply} value={baseline} />);
  fireEvent.change(screen.getByLabelText("Cor do fundo"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );

  view.rerender(<Harness onApply={onApply} value={changed} />);

  await waitFor(() =>
    expect(screen.getByLabelText("Cor do fundo")).toHaveValue("#f7f5f0"),
  );
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled();
});

test("preserves edits made after submit while the applied projection arrives", async () => {
  let finishApply!: (completed: boolean) => void;
  const pendingApply = new Promise<boolean>((resolve) => {
    finishApply = resolve;
  });
  const onApply = vi.fn<ComponentProps<typeof AlbumDesignForm>["onApply"]>(
    () => pendingApply,
  );
  const baseline = representativeProjection.state.album.visualDefaults;
  const applied: ProjectedVisualDefaults = {
    ...cloneVisualDefaults(baseline),
    background: {
      scope: "bothSides",
      both: { kind: "color", rgb: "#F7F5F0" },
    },
  };
  const view = render(<Harness onApply={onApply} value={baseline} />);

  fireEvent.change(screen.getByLabelText("Cor do fundo"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeDisabled();

  fireEvent.change(screen.getByLabelText("Cor do fundo"), {
    target: { value: "#ffffff" },
  });
  view.rerender(<Harness onApply={onApply} value={applied} />);

  await waitFor(() =>
    expect(screen.getByLabelText("Cor do fundo")).toHaveValue("#ffffff"),
  );
  await act(async () => {
    finishApply(true);
    await pendingApply;
  });
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled();
});

test("keeps the post-submit Design delta through temporarily matching predecessors", async () => {
  let finishApply!: (completed: boolean) => void;
  const pendingApply = new Promise<boolean>((resolve) => {
    finishApply = resolve;
  });
  const onApply = vi.fn<ComponentProps<typeof AlbumDesignForm>["onApply"]>(
    () => pendingApply,
  );
  const baseline = representativeProjection.state.album.visualDefaults;
  const submittedTarget: ProjectedVisualDefaults = {
    ...cloneVisualDefaults(baseline),
    background: {
      scope: "bothSides",
      both: { kind: "color", rgb: "#F7F5F0" },
    },
  };
  const matchingPostEdit: ProjectedVisualDefaults = {
    ...cloneVisualDefaults(baseline),
    frameBorder: { kind: "solid", rgb: "#2C2924", widthUm: 1_000 },
  };
  const view = render(<Harness onApply={onApply} value={baseline} />);

  fireEvent.change(screen.getByLabelText("Cor do fundo"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());

  view.rerender(
    <Harness
      onApply={onApply}
      revision={representativeProjection.state.revision + 1}
      value={submittedTarget}
    />,
  );
  fireEvent.change(screen.getByLabelText("Espessura da borda"), {
    target: { value: "1000" },
  });

  view.rerender(
    <Harness
      onApply={onApply}
      revision={representativeProjection.state.revision + 2}
      value={matchingPostEdit}
    />,
  );
  fireEvent.change(screen.getByLabelText("Cor do fundo"), {
    target: { value: "#aabbcc" },
  });

  view.rerender(
    <Harness
      onApply={onApply}
      revision={representativeProjection.state.revision + 3}
      value={submittedTarget}
    />,
  );
  await act(async () => {
    finishApply(true);
    await pendingApply;
  });

  await waitFor(() =>
    expect(screen.getByLabelText("Cor do fundo")).toHaveValue(
      "#aabbcc",
    ),
  );
  expect(screen.getByLabelText("Espessura da borda")).toHaveValue("1000");
});

test("restores the complete Design intent when Apply fails after a predecessor", async () => {
  let finishApply!: (completed: boolean) => void;
  const pendingApply = new Promise<boolean>((resolve) => {
    finishApply = resolve;
  });
  const onApply = vi.fn<ComponentProps<typeof AlbumDesignForm>["onApply"]>(
    () => pendingApply,
  );
  const baseline = representativeProjection.state.album.visualDefaults;
  const predecessor: ProjectedVisualDefaults = {
    ...cloneVisualDefaults(baseline),
    overlay: {
      scope: "bothSides",
      both: { kind: "media", mediaId: "history-overlay" },
    },
  };
  const view = render(<Harness onApply={onApply} value={baseline} />);

  fireEvent.change(screen.getByLabelText("Cor do fundo"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());
  view.rerender(
    <Harness
      onApply={onApply}
      revision={representativeProjection.state.revision + 1}
      value={predecessor}
    />,
  );

  await act(async () => {
    finishApply(false);
    await pendingApply;
  });

  expect(screen.getByLabelText("Cor do fundo")).toHaveValue("#f7f5f0");
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
});

test("records a post-submit reset against the concurrent value shown to the user", async () => {
  const onApply = vi.fn<ComponentProps<typeof AlbumDesignForm>["onApply"]>(
    () => new Promise<boolean>(() => undefined),
  );
  const baseline = representativeProjection.state.album.visualDefaults;
  const withConcurrentBorder: ProjectedVisualDefaults = {
    ...cloneVisualDefaults(baseline),
    frameBorder: { kind: "solid", rgb: "#445566", widthUm: 2_000 },
  };
  const withAnotherPredecessor: ProjectedVisualDefaults = {
    ...cloneVisualDefaults(withConcurrentBorder),
    overlay: {
      scope: "bothSides",
      both: { kind: "media", mediaId: "history-overlay" },
    },
  };
  const view = render(<Harness onApply={onApply} value={baseline} />);

  fireEvent.change(screen.getByLabelText("Cor do fundo"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());

  view.rerender(
    <Harness
      onApply={onApply}
      revision={representativeProjection.state.revision + 1}
      value={withConcurrentBorder}
    />,
  );
  await waitFor(() =>
    expect(screen.getByLabelText("Espessura da borda")).toHaveValue("2000"),
  );
  fireEvent.change(screen.getByLabelText("Espessura da borda"), {
    target: { value: "0" },
  });

  view.rerender(
    <Harness
      onApply={onApply}
      revision={representativeProjection.state.revision + 2}
      value={withAnotherPredecessor}
    />,
  );

  await waitFor(() =>
    expect(screen.getByLabelText("Espessura da borda")).toHaveValue("0"),
  );
});
