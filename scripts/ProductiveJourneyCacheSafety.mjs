import {
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  realpathSync,
  rmdirSync,
  unlinkSync,
} from "node:fs";
import path from "node:path";

function comparablePath(candidate) {
  const resolved = path.resolve(candidate);
  return process.platform === "win32" ? resolved.toLowerCase() : resolved;
}

function pathComponents(candidate) {
  const resolved = path.resolve(candidate);
  const { root } = path.parse(resolved);
  const segments = path.relative(root, resolved).split(path.sep).filter(Boolean);
  const components = [root];
  for (const segment of segments) {
    components.push(path.join(components.at(-1), segment));
  }
  return components;
}

function assertUnredirectedPath(candidate) {
  for (const component of pathComponents(candidate)) {
    const metadata = lstatSync(component, { throwIfNoEntry: false });
    if (!metadata) break;
    if (metadata.isSymbolicLink()) {
      throw new Error(
        `The isolated Cache path contains a redirected/reparse component: ${component}`,
      );
    }
    if (comparablePath(realpathSync.native(component)) !== comparablePath(component)) {
      throw new Error(
        `The isolated Cache path resolves through a redirected/reparse component: ${component}`,
      );
    }
  }
}

function assertRegularTree(current, summary) {
  for (const entry of readdirSync(current, { withFileTypes: true })) {
    const entryPath = path.join(current, entry.name);
    const metadata = lstatSync(entryPath);
    summary.entryCount += 1;
    if (metadata.isSymbolicLink()) {
      throw new Error("The isolated Cache contains a redirected/reparse entry");
    }
    if (metadata.isDirectory()) {
      assertRegularTree(entryPath, summary);
      continue;
    }
    if (!metadata.isFile()) {
      throw new Error("The isolated Cache contains a non-regular entry");
    }
    summary.byteCount += metadata.size;
    if (/\.jpe?g$/i.test(entry.name)) summary.jpegCount += 1;
  }
}

function removeRegularTree(current) {
  for (const entry of readdirSync(current, { withFileTypes: true })) {
    const entryPath = path.join(current, entry.name);
    const metadata = lstatSync(entryPath);
    if (metadata.isSymbolicLink()) {
      throw new Error(
        "The isolated Cache purge refused a redirected/reparse entry",
      );
    }
    if (metadata.isFile()) {
      unlinkSync(entryPath);
      continue;
    }
    if (!metadata.isDirectory()) {
      throw new Error("The isolated Cache purge refused a non-regular entry");
    }
    removeRegularTree(entryPath);
    const emptyDirectory = lstatSync(entryPath);
    if (emptyDirectory.isSymbolicLink() || !emptyDirectory.isDirectory()) {
      throw new Error(
        "The isolated Cache purge refused a redirected/reparse directory",
      );
    }
    rmdirSync(entryPath);
  }
}

export function createOwnedCacheGuard({ scratch, processDataRoot }) {
  const resolvedScratch = path.resolve(scratch);
  const resolvedProcessDataRoot = path.resolve(processDataRoot);
  const expected = path.resolve(
    resolvedProcessDataRoot,
    "Local",
    "MyAlbuns2",
    "Cache",
  );
  const processDataRelative = path.relative(
    resolvedScratch,
    resolvedProcessDataRoot,
  );
  if (
    !processDataRelative ||
    processDataRelative.startsWith("..") ||
    path.isAbsolute(processDataRelative)
  ) {
    throw new Error("The productive process-data root escaped its scratch root");
  }

  function assertOwnedCacheRoot(directory) {
    if (comparablePath(directory) !== comparablePath(expected)) {
      throw new Error("The Cache purge target is not the isolated productive root");
    }
    const relative = path.relative(resolvedScratch, expected);
    if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) {
      throw new Error("The Cache purge target escaped the productive scratch root");
    }
    assertUnredirectedPath(expected);
  }

  function summarizeOwnedCache(directory) {
    assertOwnedCacheRoot(directory);
    const summary = { entryCount: 0, byteCount: 0, jpegCount: 0 };
    if (!existsSync(directory)) return summary;
    const rootMetadata = lstatSync(directory);
    if (!rootMetadata.isDirectory() || rootMetadata.isSymbolicLink()) {
      throw new Error("The isolated Cache root is redirected/reparse or not a directory");
    }
    assertRegularTree(directory, summary);
    return summary;
  }

  function purgeOwnedCache(directory) {
    assertOwnedCacheRoot(directory);
    if (!existsSync(directory)) {
      mkdirSync(directory, { recursive: true });
      assertOwnedCacheRoot(directory);
    }
    summarizeOwnedCache(directory);
    assertOwnedCacheRoot(directory);
    removeRegularTree(directory);
    return summarizeOwnedCache(directory);
  }

  return { purgeOwnedCache, summarizeOwnedCache };
}
