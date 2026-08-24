import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
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
  onApply: (
    value: ProjectedVisualDefaults,
  ) => void | Promise<unknown>;
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
  const onApply = vi.fn<(value: ProjectedVisualDefaults) => void>();
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

test("resets the draft when the authoritative Album design really changes", async () => {
  const onApply = vi.fn<(value: ProjectedVisualDefaults) => void>();
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
    expect(screen.getByLabelText("Cor do Background")).toHaveValue("#112233"),
  );
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeDisabled();
});
