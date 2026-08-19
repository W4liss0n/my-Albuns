import { render } from "@testing-library/react";
import { expect, test } from "vitest";

import { SheetGuideLayer } from "./SheetGuideLayer";

test.each([
  { bleedUm: 0, safetyUm: 5_000, expectedGuideCount: 1 },
  { bleedUm: 3_000, safetyUm: 0, expectedGuideCount: 1 },
  { bleedUm: 0, safetyUm: 0, expectedGuideCount: 0 },
])(
  "does not render a zero-valued guide (%o)",
  ({ bleedUm, safetyUm, expectedGuideCount }) => {
    const { container } = render(
      <svg>
        <SheetGuideLayer
          geometry={{
            bleedUm,
            heightUm: 300_000,
            safetyUm,
            widthUm: 600_000,
          }}
        />
      </svg>,
    );

    expect(container.querySelectorAll("rect")).toHaveLength(
      expectedGuideCount,
    );
  },
);
