import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { WorkspacePanelSplitter } from "./workspacePanelLayout";

test("retains separator semantics and keyboard resize on both axes", () => {
  const resizeBy = vi.fn();
  render(
    <>
      <WorkspacePanelSplitter
        onResizeBy={resizeBy}
        onResizeStart={vi.fn()}
        panel="inspector"
        size={310}
      />
      <WorkspacePanelSplitter
        onResizeBy={resizeBy}
        onResizeStart={vi.fn()}
        panel="media"
        size={202}
      />
    </>,
  );

  const inspector = screen.getByRole("separator", {
    name: "Redimensionar painel contextual",
  });
  const media = screen.getByRole("separator", {
    name: "Redimensionar painel de imagens",
  });
  expect(inspector).toHaveAttribute("aria-orientation", "vertical");
  expect(media).toHaveAttribute("aria-orientation", "horizontal");
  fireEvent.keyDown(inspector, { key: "ArrowLeft" });
  fireEvent.keyDown(media, { key: "ArrowUp" });
  expect(resizeBy).toHaveBeenNthCalledWith(1, "inspector", 12);
  expect(resizeBy).toHaveBeenNthCalledWith(2, "media", 12);
});
