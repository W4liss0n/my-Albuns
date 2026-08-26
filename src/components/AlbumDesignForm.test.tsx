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
  value,
}: {
  onApply: ComponentProps<typeof AlbumDesignForm>["onApply"];
  value: ProjectedVisualDefaults;
}) {
  const [ready, setReady] = useState(false);
  return (
    <>
      <AlbumDesignForm
        document={representativeProjection.state.document}
        formId="album-design-equivalent-projection"
        mediaItems={[]}
        mediaPreviewUrls={{}}
        presentationUnit="mm"
        revision={representativeProjection.state.revision}
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

test("preserves an unapplied draft across a semantically equivalent projection", async () => {
  const onApply = vi.fn<ComponentProps<typeof AlbumDesignForm>["onApply"]>();
  const baseline = representativeProjection.state.album.visualDefaults;
  const view = render(<Harness onApply={onApply} value={baseline} />);
  fireEvent.change(screen.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );

  view.rerender(
    <Harness onApply={onApply} value={cloneVisualDefaults(baseline)} />,
  );

  expect(screen.getByLabelText("Cor do Background")).toHaveValue("#f7f5f0");
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled();
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
  fireEvent.change(screen.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );

  view.rerender(<Harness onApply={onApply} value={changed} />);

  await waitFor(() =>
    expect(screen.getByLabelText("Cor do Background")).toHaveValue("#f7f5f0"),
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

  fireEvent.change(screen.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeDisabled();

  fireEvent.change(screen.getByLabelText("Cor do Background"), {
    target: { value: "#ffffff" },
  });
  view.rerender(<Harness onApply={onApply} value={applied} />);

  await waitFor(() =>
    expect(screen.getByLabelText("Cor do Background")).toHaveValue("#ffffff"),
  );
  await act(async () => {
    finishApply(true);
    await pendingApply;
  });
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled();
});
