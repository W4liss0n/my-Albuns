// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readdirSync, readFileSync } from "node:fs";
// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import path from "node:path";

import * as ts from "typescript";
import { expect, test } from "vitest";

type ArchitecturalLayer = "application" | "domain" | "ui";

const sourceRoot = path.resolve("src");
const allowedDependencies: Record<ArchitecturalLayer, Set<ArchitecturalLayer>> = {
  domain: new Set(["domain"]),
  application: new Set(["application", "domain"]),
  ui: new Set(["ui", "application", "domain"]),
};

function discoverSourceFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(
    (entry: { isDirectory(): boolean; name: string }) => {
      const candidate = path.join(directory, entry.name);
      if (entry.isDirectory()) return discoverSourceFiles(candidate);
      return /\.(?:ts|tsx)$/u.test(entry.name) ? [candidate] : [];
    },
  );
}

function layerOf(candidate: string): ArchitecturalLayer | "feature" {
  const relative = path.relative(sourceRoot, path.resolve(candidate));
  for (const segment of relative.split(path.sep)) {
    if (
      segment === "application" ||
      segment === "domain" ||
      segment === "ui"
    ) {
      return segment;
    }
  }
  return "feature";
}

function relativeImports(sourcePath: string): string[] {
  const source = ts.createSourceFile(
    sourcePath,
    readFileSync(sourcePath, "utf8"),
    ts.ScriptTarget.Latest,
    true,
  );
  const imports: string[] = [];
  source.forEachChild((node) => {
    if (
      (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
      node.moduleSpecifier &&
      ts.isStringLiteral(node.moduleSpecifier) &&
      node.moduleSpecifier.text.startsWith(".")
    ) {
      imports.push(node.moduleSpecifier.text);
    }
  });
  return imports;
}

test("recognizes architectural layers nested inside feature directories", () => {
  expect(
    layerOf(path.join(sourceRoot, "global", "application", "example.ts")),
  ).toBe("application");
  expect(
    layerOf(path.join(sourceRoot, "project-dialog", "application", "example.ts")),
  ).toBe("application");
});

test("keeps domain, application, and shared UI dependencies pointing inward", () => {
  const violations: string[] = [];
  for (const sourcePath of discoverSourceFiles(sourceRoot)) {
    const sourceLayer = layerOf(sourcePath);
    if (sourceLayer === "feature") continue;
    for (const specifier of relativeImports(sourcePath)) {
      const targetPath = path.resolve(path.dirname(sourcePath), specifier);
      const targetLayer = layerOf(targetPath);
      if (
        targetLayer === "feature" ||
        !allowedDependencies[sourceLayer].has(targetLayer)
      ) {
        violations.push(
          `${path.relative(sourceRoot, sourcePath)} -> ${specifier}`,
        );
      }
    }
  }

  expect(violations).toEqual([]);
});
