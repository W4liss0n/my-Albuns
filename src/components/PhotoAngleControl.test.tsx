import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { PhotoAngleControl } from "./PhotoAngleControl";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function harness(value: number | null = 120, doubleClickTimeMs = 900) {
  const onPreview = vi.fn();
  const onCommit = vi.fn();
  const onCancel = vi.fn();
  const actions = { disabled: false, doubleClickTimeMs, dragThreshold: { x: 5, y: 5 },
    scopeKey: "project:photo-1", onPreview, onCommit, onCancel };
  const view = render(<PhotoAngleControl value={value} {...actions} />);
  Object.defineProperty(screen.getByRole("slider"), "setPointerCapture", { value: vi.fn() });
  return { view, actions, onPreview, onCommit, onCancel,
    slider: screen.getByRole("slider", { name: "Ângulo da Foto" }),
    number: screen.getByRole("spinbutton", { name: "Ângulo em graus" }) };
}

test("previews every drag sample and commits once on release", () => {
  const h = harness();
  fireEvent.pointerDown(h.slider, { pointerId: 1, button: 0, clientX: 20, clientY: 8 });
  fireEvent.change(h.slider, { target: { value: "14.2" } });
  fireEvent.pointerMove(h.slider, { pointerId: 1, clientX: 60, clientY: 8 });
  fireEvent.change(h.slider, { target: { value: "18.3" } });
  expect(h.onPreview.mock.calls).toEqual([[142], [183]]);
  expect(h.onCommit).not.toHaveBeenCalled();
  fireEvent.pointerUp(h.slider, { pointerId: 1 });
  fireEvent.lostPointerCapture(h.slider, { pointerId: 1 });
  expect(h.onCommit).toHaveBeenCalledExactlyOnceWith(183);
  expect(h.onCancel).not.toHaveBeenCalled();
});

test("two off-center clicks reset once and respect the Windows double-click interval", () => {
  const h = harness();
  const click = (value: string) => {
    fireEvent.pointerDown(h.slider, { pointerId: 1, button: 0, clientX: 80, clientY: 8 });
    fireEvent.change(h.slider, { target: { value } });
    fireEvent.pointerUp(h.slider, { pointerId: 1 });
    fireEvent.lostPointerCapture(h.slider, { pointerId: 1 });
    fireEvent.click(h.slider);
  };
  click("23.1");
  act(() => vi.advanceTimersByTime(700));
  expect(h.onCommit).not.toHaveBeenCalled();
  click("23.2");
  fireEvent.doubleClick(h.slider);
  act(() => vi.runAllTimers());
  expect(h.onCommit).toHaveBeenCalledExactlyOnceWith(0);
  expect(h.number).toHaveValue("0");
});

test("accepts comma or dot, validates precision and range, and cancels invalid edits", () => {
  const h = harness();
  fireEvent.change(h.number, { target: { value: "-12,3" } });
  expect(h.onPreview).toHaveBeenLastCalledWith(-123);
  fireEvent.keyDown(h.number, { key: "Enter" });
  expect(h.onCommit).toHaveBeenCalledExactlyOnceWith(-123);
  for (const value of ["", "-", "45.1", "-45.1", "1.23", "NaN", "1e1"]) {
    fireEvent.change(h.number, { target: { value } });
    expect(h.number).toHaveAttribute("aria-invalid", "true");
    fireEvent.blur(h.number);
  }
  expect(h.onCommit).toHaveBeenCalledTimes(1);
  fireEvent.change(h.number, { target: { value: "45.0" } });
  fireEvent.blur(h.number);
  expect(h.onCommit).toHaveBeenLastCalledWith(450);
});

test("mixed values remain indeterminate until edited; Escape and capture loss discard previews", () => {
  const h = harness(null);
  expect(h.number).toHaveValue("");
  expect(h.number).toHaveAttribute("placeholder", "—");
  expect(h.slider).toHaveAttribute("aria-valuetext", "Múltiplos valores");
  expect(h.slider).toHaveAttribute("data-mixed", "true");
  fireEvent.focus(h.number);
  fireEvent.blur(h.number);
  expect(h.onCommit).not.toHaveBeenCalled();
  fireEvent.change(h.number, { target: { value: "2.1" } });
  fireEvent.keyDown(h.number, { key: "Escape" });
  fireEvent.blur(h.number);
  expect(h.number).toHaveValue("");
  expect(h.onCommit).not.toHaveBeenCalled();
  fireEvent.pointerDown(h.slider, { pointerId: 2, button: 0, clientX: 20 });
  fireEvent.change(h.slider, { target: { value: "4" } });
  fireEvent.lostPointerCapture(h.slider, { pointerId: 2 });
  act(() => vi.runAllTimers());
  expect(h.onCommit).not.toHaveBeenCalled();
  expect(h.onCancel).toHaveBeenCalled();
});

test("keyboard changes form one edit and leaving the control flushes a pending click", () => {
  const h = harness();
  fireEvent.keyDown(h.slider, { key: "ArrowRight" });
  fireEvent.change(h.slider, { target: { value: "12.1" } });
  fireEvent.keyDown(h.slider, { key: "ArrowRight", repeat: true });
  fireEvent.change(h.slider, { target: { value: "12.2" } });
  fireEvent.keyUp(h.slider, { key: "ArrowRight" });
  expect(h.onCommit).toHaveBeenCalledExactlyOnceWith(122);
  fireEvent.pointerDown(h.slider, { pointerId: 1, button: 0, clientX: 20 });
  fireEvent.change(h.slider, { target: { value: "22" } });
  fireEvent.pointerUp(h.slider, { pointerId: 1 });
  fireEvent.pointerDown(document.body, { button: 0 });
  act(() => vi.runAllTimers());
  expect(h.onCommit.mock.calls).toEqual([[122], [220]]);
});

test("unmount discards the pending click without modifying the Project", () => {
  const h = harness();
  fireEvent.pointerDown(h.slider, { pointerId: 1, button: 0 });
  fireEvent.change(h.slider, { target: { value: "34" } });
  fireEvent.pointerUp(h.slider, { pointerId: 1 });
  h.view.unmount();
  act(() => vi.runAllTimers());
  expect(h.onCommit).not.toHaveBeenCalled();
  expect(h.onCancel).toHaveBeenCalled();
});
