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

function observableDomRepresentations(pageSource) {
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
