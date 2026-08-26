import { expect, test } from "vitest";

import type {
  AlbumInformation,
  EditorProjection,
  ProjectedVisualDefaults,
} from "../domain/project";
import {
  createAlbumDesignProjectDraft,
  createAlbumInformationProjectDraft,
} from "./projectSettingsDraft";

const baselineInformation: AlbumInformation = {
  displayUnit: "mm",
  sheetWidthUm: 600_000,
  sheetHeightUm: 300_000,
  dpi: 300,
  bleedUm: 3_000,
  safetyUm: 3_000,
  firstSheet: "double",
  lastSheet: "double",
};

const baselineVisualDefaults: ProjectedVisualDefaults = {
  background: {
    scope: "bothSides",
    both: { kind: "color", rgb: "#FFFFFF" },
  },
  overlay: { scope: "bothSides", both: null },
  frameBorder: { kind: "none" },
};

const representativeProjection: EditorProjection = {
  state: {
    projectId: "draft-contract-project",
    projectName: "Contrato de draft",
    document: {
      displayUnit: "mm",
      sheetWidthUm: 600_000,
      sheetHeightUm: 300_000,
      dpi: 300,
      bleedUm: 3_000,
      safetyUm: 3_000,
    },
    album: {
      sheets: [
        {
          id: "sheet-001",
          number: 1,
          role: "initial",
          activeSides: "both",
          pageNumbers: [1, 2],
          widthUm: 600_000,
          heightUm: 300_000,
          frames: [],
        },
      ],
      media: [],
      visualDefaults: baselineVisualDefaults,
    },
    revision: 25,
    savedRevision: 25,
    dirty: false,
    canUndo: true,
    canRedo: false,
  },
  composition: { frameBorder: { kind: "none" }, sheets: [] },
  mediaUsage: [],
};

test("owns Album Information baseline, revision, equality, transition and execution-time materialization", () => {
  const initial = createAlbumInformationProjectDraft(
    representativeProjection.state.revision,
    baselineInformation,
  );
  expect(initial.baselineRevision).toBe(25);
  expect(initial.changed).toBe(false);
  expect(initial.equals(baselineInformation)).toBe(true);

  const candidate = { ...baselineInformation, dpi: 600 };
  const transitioned = initial.transition(candidate);
  expect(transitioned.changed).toBe(true);
  expect(transitioned.equals(candidate)).toBe(true);
  expect(transitioned.delta).toEqual({ dpi: 600 });

  const latestProjection: EditorProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      revision: 26,
      document: {
        ...representativeProjection.state.document,
        bleedUm: 5_000,
        safetyUm: 7_000,
      },
    },
  };
  const materialized = transitioned.materializeAgainst(latestProjection);
  expect(materialized).toEqual({
    baselineRevision: 26,
    changed: true,
    baseline: {
      ...baselineInformation,
      bleedUm: 5_000,
      safetyUm: 7_000,
    },
    value: {
      ...baselineInformation,
      dpi: 600,
      bleedUm: 5_000,
      safetyUm: 7_000,
    },
    intent: {
      kind: "setAlbumInformation",
      information: {
        ...baselineInformation,
        dpi: 600,
        bleedUm: 5_000,
        safetyUm: 7_000,
      },
    },
  });
  expect(transitioned.materialize(latestProjection)).toEqual({
    kind: "setAlbumInformation",
    information: {
      ...baselineInformation,
      dpi: 600,
      bleedUm: 5_000,
      safetyUm: 7_000,
    },
  });
});

test("replays only the changed Album Design side over the latest scoped values", () => {
  const baseline = representativeProjection.state.album.visualDefaults;
  const candidate: ProjectedVisualDefaults = {
    ...baseline,
    background: {
      scope: "perSide",
      left: { kind: "color", rgb: "#F7F5F0" },
      right: { kind: "color", rgb: "#FFFFFF" },
    },
  };
  const draft = createAlbumDesignProjectDraft(25, baseline).transition(
    candidate,
  );
  expect(draft.baselineRevision).toBe(25);
  expect(draft.changed).toBe(true);
  expect(draft.equals(candidate)).toBe(true);
  expect(draft.delta).toEqual({
    background: {
      kind: "sides",
      left: { kind: "color", rgb: "#F7F5F0" },
    },
    overlay: { kind: "none" },
  });

  const latestProjection: EditorProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      revision: 24,
      album: {
        ...representativeProjection.state.album,
        visualDefaults: {
          background: {
            scope: "perSide",
            left: { kind: "color", rgb: "#AABBCC" },
            right: { kind: "color", rgb: "#223344" },
          },
          overlay: {
            scope: "bothSides",
            both: { kind: "media", mediaId: "history-overlay" },
          },
          frameBorder: {
            kind: "solid",
            rgb: "#445566",
            widthUm: 2_000,
          },
        },
      },
    },
  };
  expect(draft.materialize(latestProjection)).toEqual({
    kind: "setVisualDefaults",
    visualDefaults: {
      background: {
        scope: "perSide",
        left: { kind: "color", rgb: "#F7F5F0" },
        right: { kind: "color", rgb: "#223344" },
      },
      overlay: latestProjection.state.album.visualDefaults.overlay,
      frameBorder: latestProjection.state.album.visualDefaults.frameBorder,
    },
  });
});

test("preserves the selected side scope without replacing the opposite side changed by History", () => {
  const candidate: ProjectedVisualDefaults = {
    ...baselineVisualDefaults,
    overlay: { scope: "perSide", left: null, right: null },
  };
  const draft = createAlbumDesignProjectDraft(
    representativeProjection.state.revision,
    baselineVisualDefaults,
  ).transition(candidate);
  const latestProjection: EditorProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      revision: representativeProjection.state.revision + 1,
      album: {
        ...representativeProjection.state.album,
        visualDefaults: {
          ...baselineVisualDefaults,
          overlay: {
            scope: "perSide",
            left: null,
            right: { kind: "media", mediaId: "history-overlay" },
          },
        },
      },
    },
  };

  expect(draft.materialize(latestProjection)).toEqual({
    kind: "setVisualDefaults",
    visualDefaults: {
      ...baselineVisualDefaults,
      overlay: {
        scope: "perSide",
        left: null,
        right: { kind: "media", mediaId: "history-overlay" },
      },
    },
  });
});

test("recognizes when History has already materialized the Album Design intent", () => {
  const target: ProjectedVisualDefaults = {
    ...baselineVisualDefaults,
    background: {
      scope: "bothSides",
      both: { kind: "color", rgb: "#F7F5F0" },
    },
  };
  const draft = createAlbumDesignProjectDraft(
    representativeProjection.state.revision,
    baselineVisualDefaults,
  ).transition(target);
  const alreadyMaterialized: ProjectedVisualDefaults = {
    ...target,
    overlay: {
      scope: "bothSides",
      both: { kind: "media", mediaId: "history-overlay" },
    },
  };
  const latestProjection: EditorProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      revision: representativeProjection.state.revision + 1,
      canRedo: true,
      album: {
        ...representativeProjection.state.album,
        visualDefaults: alreadyMaterialized,
      },
    },
  };

  expect(draft.materializeAgainst(latestProjection).changed).toBe(false);
});
