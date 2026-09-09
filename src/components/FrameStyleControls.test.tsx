import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { DisplayUnit } from "../domain/project";
import { frameStyleCorpus } from "../test/frameStylePreview";
import { FrameStyleControls } from "./FrameStyleControls";

test.each<[DisplayUnit, string, string]>([["mm", "mm", "2,54"], ["cm", "cm", "0,254"], ["in", "pol", "0,1"]])(
  "a border measurement in %s commits the same physical width", (unit, label, value) => {
    const onCommit = vi.fn();
    render(<FrameStyleControls frames={frameStyleCorpus.states.album.state.album.sheets[0].frames}
      unit={unit} disabled={false} scopeKey="physical" doubleClickTimeMs={500} dragThreshold={{ x: 5, y: 5 }}
      onPreview={vi.fn()} onCommit={onCommit} onCancel={vi.fn()} />);
    const field = screen.getByRole("spinbutton", { name: `Espessura da Borda em ${label}` });
    fireEvent.change(field, { target: { value } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onCommit).toHaveBeenCalledExactlyOnceWith({ kind: "borderWidth", widthUm: 2_540 });
  },
);
