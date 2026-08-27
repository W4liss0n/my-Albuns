import assert from "node:assert/strict";
import test from "node:test";

import { assertOriginalPathsRemainOutsideWebView } from "./RealCanvasGatePrivacy.mjs";

const originalPaths = [
  String.raw`C:\Users\Ana & João\Álbuns\background-original.png`,
  String.raw`\\nas-01\Fotos Família\overlay-original.png`,
];

test("accepts Original basenames and display names in the productive DOM", () => {
  assert.doesNotThrow(() =>
    assertOriginalPathsRemainOutsideWebView(
      `<main>
        <span aria-label="background-original.png">background-original.png</span>
        <span data-display-name="overlay-original.png">overlay-original.png</span>
        <canvas data-texture="myalbuns-preview://opaque/resource-01"></canvas>
      </main>`,
      originalPaths,
    ),
  );
});

test("rejects directory-qualified Windows and UNC Original paths in DOM forms", () => {
  const exposedSources = [
    {
      label: "Windows absolute attribute",
      source: String.raw`<img data-source="C:\Users\Ana & João\Álbuns\background-original.png">`,
    },
    {
      label: "HTML-escaped Windows text",
      source: String.raw`<p>C:\Users\Ana &amp; João\Álbuns\background-original.png</p>`,
    },
    {
      label: "forward-slash file URL",
      source: `<img style="width: 100%" src="file:///C:/Users/Ana%20%26%20Jo%C3%A3o/%C3%81lbuns/background-original.png">`,
    },
    {
      label: "Original directory without basename",
      source: String.raw`<output data-directory="C:\Users\Ana & João\Álbuns"></output>`,
    },
    {
      label: "Windows drive root without basename",
      source: String.raw`<output data-directory="C:\"></output>`,
    },
    {
      label: "directory-qualified relative path",
      source: `<span data-source="Álbuns/background-original.png"></span>`,
    },
    {
      label: "JSON-escaped Windows path in a DOM script",
      source: String.raw`<script type="application/json">{"path":"C:\\Users\\Ana & João\\Álbuns\\background-original.png"}</script>`,
    },
    {
      label: "UNC attribute",
      source: String.raw`<img data-source="\\nas-01\Fotos Família\overlay-original.png">`,
    },
    {
      label: "UNC share root without basename",
      source: String.raw`<output data-directory="\\nas-01\Fotos Família"></output>`,
    },
    {
      label: "HTML-encoded UNC text",
      source: `<p>&#92;&#92;nas-01&#92;Fotos Família&#92;overlay-original.png</p>`,
    },
  ];

  for (const { label, source } of exposedSources) {
    assert.throws(
      () => assertOriginalPathsRemainOutsideWebView(source, originalPaths),
      {
        message: "An Original pathname crossed the productive WebView boundary",
      },
      label,
    );
  }
});
