import path from "node:path";

const PATH_EXPOSURE_ERROR =
  "An Original pathname crossed the productive WebView boundary";

function decodeHtmlEntities(value) {
  return value
    .replace(/&#x([0-9a-f]+);/giu, (_match, digits) =>
      String.fromCodePoint(Number.parseInt(digits, 16)),
    )
    .replace(/&#([0-9]+);/gu, (_match, digits) =>
      String.fromCodePoint(Number.parseInt(digits, 10)),
    )
    .replace(
      /&(amp|apos|bsol|gt|lt|quot);/giu,
      (_match, entity) =>
        ({
          amp: "&",
          apos: "'",
          bsol: "\\",
          gt: ">",
          lt: "<",
          quot: '"',
        })[entity.toLowerCase()],
    );
}

function decodePercentEscapes(value) {
  return value.replace(/(?:%[0-9a-f]{2})+/giu, (encoded) => {
    try {
      return decodeURIComponent(encoded);
    } catch {
      return encoded;
    }
  });
}

function decodedDomRepresentations(pageSource) {
  const representations = new Set([pageSource]);
  for (const current of [...representations]) {
    representations.add(decodeHtmlEntities(current));
  }
  for (const current of [...representations]) {
    representations.add(decodePercentEscapes(current));
  }
  for (const current of [...representations]) {
    representations.add(current.replaceAll("\\\\", "\\"));
  }
  return [...representations];
}

function jsonValueEnd(value, start) {
  if (value[start] === '"') {
    let escaped = false;
    for (let index = start + 1; index < value.length; index += 1) {
      if (escaped) {
        escaped = false;
      } else if (value[index] === "\\") {
        escaped = true;
      } else if (value[index] === '"') {
        return index + 1;
      }
    }
    return undefined;
  }

  const closingByOpening = { "[": "]", "{": "}" };
  if (!(value[start] in closingByOpening)) return undefined;

  const expectedClosings = [];
  let escaped = false;
  let insideString = false;
  for (let index = start; index < value.length; index += 1) {
    const character = value[index];
    if (insideString) {
      if (escaped) {
        escaped = false;
      } else if (character === "\\") {
        escaped = true;
      } else if (character === '"') {
        insideString = false;
      }
      continue;
    }

    if (character === '"') {
      insideString = true;
    } else if (character in closingByOpening) {
      expectedClosings.push(closingByOpening[character]);
    } else if (character === "]" || character === "}") {
      if (expectedClosings.pop() !== character) return undefined;
      if (expectedClosings.length === 0) return index + 1;
    }
  }
  return undefined;
}

function parsedJsonValuesInText(value) {
  const parsedValues = [];
  for (let index = 0; index < value.length; index += 1) {
    if (value[index] !== '"' && value[index] !== "[" && value[index] !== "{") {
      continue;
    }
    const end = jsonValueEnd(value, index);
    if (end === undefined) continue;
    try {
      parsedValues.push(JSON.parse(value.slice(index, end)));
      index = end - 1;
    } catch {
      // Keep scanning so a valid nested value can still be inspected.
    }
  }
  return parsedValues;
}

function decodeWindowsUtf16Units(units) {
  let decoded = "";
  for (let index = 0; index < units.length; index += 4_096) {
    decoded += String.fromCharCode(...units.slice(index, index + 4_096));
  }
  return decoded;
}

function reconstructedNativePaths(decodedRepresentations) {
  const pendingText = [...decodedRepresentations];
  const inspectedText = new Set();
  const reconstructed = new Set();

  function inspectValue(value) {
    if (typeof value === "string") {
      for (const decoded of decodedDomRepresentations(value)) {
        if (decoded.includes("{") || decoded.includes("[")) {
          pendingText.push(decoded);
        }
      }
      return;
    }
    if (Array.isArray(value)) {
      for (const item of value) inspectValue(item);
      return;
    }
    if (value === null || typeof value !== "object") return;

    if (
      value.encoding === "windowsUtf16" &&
      Array.isArray(value.units) &&
      value.units.every(
        (unit) => Number.isInteger(unit) && unit >= 0 && unit <= 0xffff,
      )
    ) {
      reconstructed.add(decodeWindowsUtf16Units(value.units));
    }
    for (const nested of Object.values(value)) inspectValue(nested);
  }

  while (pendingText.length > 0) {
    const current = pendingText.pop();
    if (inspectedText.has(current)) continue;
    inspectedText.add(current);
    for (const parsed of parsedJsonValuesInText(current)) inspectValue(parsed);
  }
  return reconstructed;
}

function observableDomRepresentations(pageSource) {
  const decodedRepresentations = decodedDomRepresentations(pageSource);
  const representations = new Set(decodedRepresentations);
  for (const reconstructed of reconstructedNativePaths(decodedRepresentations)) {
    for (const decoded of decodedDomRepresentations(reconstructed)) {
      representations.add(decoded);
    }
  }
  return [...representations].flatMap((current) => [
    current.toLowerCase(),
    current.replaceAll("\\", "/").toLowerCase(),
  ]);
}

function addDirectoryQualifiedSuffixes(fragments, candidate) {
  const segments = candidate.replaceAll("\\", "/").split("/").filter(Boolean);
  for (let index = 0; index < segments.length - 1; index += 1) {
    fragments.add(segments.slice(index).join("/"));
  }
}

function sensitivePathFragments(originalPath) {
  const fragments = new Set();
  const nativePath = String(originalPath);
  if (!nativePath) return fragments;

  fragments.add(nativePath);
  fragments.add(nativePath.replaceAll("\\", "/"));
  addDirectoryQualifiedSuffixes(fragments, nativePath);

  const directory = path.win32.dirname(nativePath);
  const root = path.win32.parse(nativePath).root;
  const windowsRoot =
    /^[a-z]:[\\/]$/iu.test(root) || root.startsWith("\\\\");
  if (windowsRoot) {
    fragments.add(root);
    fragments.add(root.replaceAll("\\", "/"));
    if (root.startsWith("\\\\")) {
      const shareRoot = root.replace(/[\\/]+$/u, "");
      fragments.add(shareRoot);
      fragments.add(shareRoot.replaceAll("\\", "/"));
    }
  }
  if (directory !== "." && directory !== root) {
    fragments.add(directory);
    fragments.add(directory.replaceAll("\\", "/"));
    addDirectoryQualifiedSuffixes(fragments, directory);
  }
  return new Set([...fragments].map((fragment) => fragment.toLowerCase()));
}

export function assertOriginalPathsRemainOutsideWebView(
  pageSource,
  originalPaths,
) {
  if (typeof pageSource !== "string" || !Array.isArray(originalPaths)) {
    throw new TypeError("The WebView privacy oracle received invalid evidence");
  }

  const observed = observableDomRepresentations(pageSource);
  for (const originalPath of originalPaths) {
    for (const fragment of sensitivePathFragments(originalPath)) {
      if (observed.some((candidate) => candidate.includes(fragment))) {
        throw new Error(PATH_EXPOSURE_ERROR);
      }
    }
  }
}
