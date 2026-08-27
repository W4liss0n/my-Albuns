import assert from "node:assert/strict";
import test from "node:test";

import { assertOriginalPathsRemainOutsideWebView } from "./RealCanvasGatePrivacy.mjs";

const originalPaths = [
  String.raw`C:\Users\Ana & João\Álbuns\background-original.png`,
  String.raw`\\nas-01\Fotos Família\overlay-original.png`,
];

function windowsUtf16Units(value) {
  return Array.from({ length: value.length }, (_unused, index) =>
    value.charCodeAt(index),
  );
}

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

test("rejects a reversible windowsUtf16 path in serialized DOM JSON", () => {
  const source = String.raw`<script type="application/json">{"path":{"encoding":"windowsUtf16","units":[67,58,92,85,115,101,114,115,92,65,110,97,32,38,32,74,111,227,111,92,193,108,98,117,110,115,92,98,97,99,107,103,114,111,117,110,100,45,111,114,105,103,105,110,97,108,46,112,110,103]}}</script>`;

  assert.throws(
    () => assertOriginalPathsRemainOutsideWebView(source, originalPaths),
    {
      message: "An Original pathname crossed the productive WebView boundary",
    },
  );
});

test("rejects a unicode-escaped windowsUtf16 discriminator", () => {
  const encodedDto = String.raw`{"encoding":"windowsUtf\u0031\u0036","units":[67,58,92,85,115,101,114,115,92,65,110,97,32,38,32,74,111,227,111,92,193,108,98,117,110,115,92,98,97,99,107,103,114,111,117,110,100,45,111,114,105,103,105,110,97,108,46,112,110,103]}`;
  const exposedSources = [
    `<script type="application/json">{"path":${encodedDto}}</script>`,
    `<script type="application/json">{"state":{"project":{"path":${encodedDto}}}}</script>`,
  ];

  for (const source of exposedSources) {
    assert.throws(
      () => assertOriginalPathsRemainOutsideWebView(source, originalPaths),
      {
        message: "An Original pathname crossed the productive WebView boundary",
      },
    );
  }
});

test("rejects a nested windowsUtf16 path inside serialized JSON text", () => {
  const source = `<script type="application/json">${JSON.stringify({
    hydration: JSON.stringify({
      project: {
        media: [
          {
            path: {
              encoding: "windowsUtf16",
              units: windowsUtf16Units(originalPaths[1]),
            },
          },
        ],
      },
    }),
  })}</script>`;

  assert.throws(
    () => assertOriginalPathsRemainOutsideWebView(source, originalPaths),
    {
      message: "An Original pathname crossed the productive WebView boundary",
    },
  );
});

test("accepts common numeric lists and display-only windowsUtf16 values", () => {
  const source = `<script type="application/json">${JSON.stringify({
    viewport: [1_920, 1_080, 300],
    codeUnitsWithoutEncoding: windowsUtf16Units(originalPaths[0]),
    media: [
      {
        displayName: "background-original.png",
        displayNameValue: {
          encoding: "windowsUtf16",
          units: windowsUtf16Units("background-original.png"),
        },
      },
    ],
    unrelatedEncodedNumbers: {
      encoding: "windowsUtf16",
      units: [1_920, 1_080, 300],
    },
    wrongEncoding: {
      encoding: "utf8",
      units: windowsUtf16Units(originalPaths[0]),
    },
    splitDtoFields: {
      encoding: "windowsUtf16",
      payload: { units: windowsUtf16Units(originalPaths[0]) },
    },
  })}</script>`;

  assert.doesNotThrow(() =>
    assertOriginalPathsRemainOutsideWebView(source, originalPaths),
  );
});

test("rejects encoded path forms reconstructed from windowsUtf16 values", () => {
  const exposedValues = [
    {
      label: "percent-encoded UNC file URL",
      value:
        "file://nas-01/Fotos%20Fam%C3%ADlia/overlay-original.png",
    },
    {
      label: "Original directory",
      value: String.raw`C:\Users\Ana & João\Álbuns`,
    },
    { label: "Windows drive root", value: "C:\\" },
    {
      label: "directory-qualified relative path",
      value: String.raw`Álbuns\background-original.png`,
    },
    {
      label: "UNC share root",
      value: String.raw`\\nas-01\Fotos Família`,
    },
    {
      label: "HTML-encoded UNC path",
      value:
        "&#92;&#92;nas-01&#92;Fotos Família&#92;overlay-original.png",
    },
    {
      label: "JSON-doubled UNC separators",
      value: String.raw`\\\\nas-01\\Fotos Família\\overlay-original.png`,
    },
  ];

  for (const { label, value } of exposedValues) {
    const source = `<script type="application/json">${JSON.stringify({
      path: {
        encoding: "windowsUtf16",
        units: windowsUtf16Units(value),
      },
    })}</script>`;
    assert.throws(
      () => assertOriginalPathsRemainOutsideWebView(source, originalPaths),
      {
        message: "An Original pathname crossed the productive WebView boundary",
      },
      label,
    );
  }
});

test("rejects a windowsUtf16 DTO serialized into an HTML attribute", () => {
  const serializedDto = JSON.stringify({
    path: {
      encoding: "windowsUtf16",
      units: windowsUtf16Units(originalPaths[1]),
    },
  }).replaceAll('"', "&quot;");

  assert.throws(
    () =>
      assertOriginalPathsRemainOutsideWebView(
        `<div data-native-path="${serializedDto}"></div>`,
        originalPaths,
      ),
    {
      message: "An Original pathname crossed the productive WebView boundary",
    },
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
