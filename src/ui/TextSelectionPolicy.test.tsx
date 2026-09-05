// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readFileSync } from "node:fs";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

const uiStyles = readFileSync("src/ui/ui.css", "utf8") as string;

test("keeps application chrome inert while editable and declared copyable text stays selectable", () => {
  render(
    <main className="ui-chrome-selection-scope" data-testid="shell">
      <span data-testid="chrome-label">Grade de Lâminas</span>
      <input aria-label="Valor numérico" defaultValue="42" type="number" />
      <textarea aria-label="Observações" defaultValue="texto" />
      <div contentEditable suppressContentEditableWarning>
        Conteúdo editável
      </div>
      <code className="ui-copyable-text">C:\Projetos\album.mya</code>
    </main>,
  );

  expect(screen.getByTestId("shell")).toHaveClass(
    "ui-chrome-selection-scope",
  );
  expect(uiStyles).toMatch(
    /\.ui-chrome-selection-scope\s*\{[^}]*user-select:\s*none;/su,
  );
  expect(uiStyles).toMatch(
    /\.ui-chrome-selection-scope\s+:where\([\s\S]*?\.ui-copyable-text[\s\S]*?\)\s*\{[^}]*user-select:\s*text;/u,
  );
  expect(screen.getByText("C:\\Projetos\\album.mya")).toHaveClass(
    "ui-copyable-text",
  );

  const textarea = screen.getByRole("textbox", {
    name: "Observações",
  }) as HTMLTextAreaElement;
  textarea.focus();
  textarea.setSelectionRange(0, 5);
  const copy = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    ctrlKey: true,
    key: "c",
  });
  fireEvent(textarea, copy);
  expect(copy.defaultPrevented).toBe(false);
  expect(textarea).toHaveProperty("selectionStart", 0);
  expect(textarea).toHaveProperty("selectionEnd", 5);
});
