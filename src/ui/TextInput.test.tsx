import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { TextInput } from "./TextInput";

test("keeps browser and WebView suggestions disabled", () => {
  render(<TextInput aria-label="Medida" />);

  expect(screen.getByRole("textbox", { name: "Medida" })).toHaveAttribute(
    "autocomplete",
    "off",
  );
});
