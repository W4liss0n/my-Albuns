// Windows-only, protocol v26. Run with both EXEs, five-photo directory, and output directory.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';

assert.equal(process.platform, 'win32');
assert.equal(process.argv.length, 6, 'Expected baseline.exe optimized.exe photos-directory output-directory');
const [baseline, optimized, photosDirectory, output] = process.argv.slice(2).map(value => path.resolve(value));
mkdirSync(output, { recursive: true });
const hash = value => createHash('sha256').update(value).digest('hex');
const fileHash = file => hash(readFileSync(file));
const nativePath = value => ({ encoding: 'windowsUtf16', units: Array.from({ length: value.length }, (_, i) => value.charCodeAt(i)) });
const inputs = Array.from({ length: 5 }, (_, i) => path.join(photosDirectory, `photo-${i + 1}.jpg`));
const mediaIds = inputs.map(() => randomUUID());
const cacheRoot = path.join(process.env.LOCALAPPDATA, 'MyAlbuns2', 'Cache', `release-benchmark-${randomUUID()}`);
const roots = [...new Set([...inputs, output, cacheRoot].map(value => path.parse(value).root))];
assert.ok(roots.every(root => /^[A-Z]:\\$/i.test(root)), 'Use local disk paths for this experiment');
const rootBindings = { bindings: roots.map(root => ({ kind: 'disk', logicalRoot: nativePath(root), operationalRoot: nativePath(root) })) };
const policy = { representationVersion: 1, maxEdgePx: 1600, maxDecodedPixels: 134217728, maxDecoderAllocBytes: 536870912, jpegQuality: 84, acceptSinglePageTiff: true, rejectMultiPageTiff: true, embedSrgbProfile: true };
const common = { protocolVersion: 26, projectId: 'release-benchmark', rootBindings };
const samples = [];
const expectedHashes = new Map();
const report = {
  collectedAt: new Date().toISOString(), protocolVersion: 26,
  workload: 'Five JPEG previews; 600 x 300 mm sheet with five decorative images, JPEG at 300 dpi',
  methodology: 'Two warmups and eight timed samples per variant/workload; alternating order; wall time includes process startup; output hashing excluded',
  cacheRoot, inputs: inputs.map(file => ({ name: path.basename(file), bytes: statSync(file).size, sha256: fileHash(file) })),
  binaries: { baseline: { bytes: statSync(baseline).size, sha256: fileHash(baseline) }, optimized: { bytes: statSync(optimized).size, sha256: fileHash(optimized) } },
  samples, passed: false,
};
const rect = { x: 0, y: 0, width: 600000, height: 300000 };
const unit = {
  frameBorder: { kind: 'none' },
  sheet: { sheetId: 'benchmark-sheet', number: 1, activeSides: 'both', widthUm: rect.width, heightUm: rect.height,
    base: { rgb: '#FFFFFF', drawRect: rect }, frames: [], overlays: [],
    backgrounds: inputs.map((_, i) => ({ kind: 'media', mediaId: mediaIds[i], name: `Photo ${i + 1}`,
      drawRect: { x: i * 120000, y: 0, width: 120000, height: 300000 } })),
  },
};
function run(executable, args, payload) {
  const start = performance.now();
  const result = spawnSync(executable, args, {
    input: payload === undefined ? undefined : `${JSON.stringify(payload)}\n`, encoding: 'utf8',
    windowsHide: true, timeout: 120000, maxBuffer: 4 * 1024 * 1024,
  });
  const milliseconds = performance.now() - start;
  assert.equal(result.error, undefined);
  assert.equal(result.status, 0, result.stderr);
  return { stdout: result.stdout, milliseconds };
}
function compareHashes(workload, hashes) {
  if (!expectedHashes.has(workload)) expectedHashes.set(workload, hashes);
  assert.deepEqual(hashes, expectedHashes.get(workload), `${workload}: output bytes changed`);
}
try {
  for (const [variant, executable] of [['baseline', baseline], ['optimized', optimized]]) {
    assert.equal(run(executable, ['--protocol-version']).stdout.trim(), '26', variant);
  }
  for (const workload of ['processor-startup', 'photo-import', 'jpeg-render']) {
    for (let round = 0; round < 10; round++) {
      const variants = round % 2 ? [['optimized', optimized], ['baseline', baseline]] : [['baseline', baseline], ['optimized', optimized]];
      for (const [variant, executable] of variants) {
        const requestId = `${workload}-${variant}-${round}`;
        let measured;
        if (workload === 'processor-startup') {
          measured = run(executable, ['--protocol-version']);
          assert.equal(measured.stdout.trim(), '26');
        } else {
          const destination = path.join(output, `${requestId}.jpg`);
          const payload = workload === 'photo-import' ? {
            kind: 'preparePhotoImport', request: { ...common, requestId, attemptId: requestId, cachePaths: { root: cacheRoot }, policy,
              candidates: inputs.map((source, i) => ({ sourceId: `photo-${i + 1}`, sourcePath: nativePath(source), generationId: requestId })) },
          } : {
            kind: 'render', request: { ...common, requestId, revision: 0, preparedOutputPath: nativePath(destination), dpi: 300, unit,
              sources: inputs.map((source, i) => ({ mediaId: mediaIds[i], sourcePath: nativePath(source) })) },
          };
          measured = run(executable, [], payload);
          const events = measured.stdout.trim().split('\n').map(line => JSON.parse(line));
          const response = events.at(-1);
          assert.equal(response.kind, 'response');
          assert.equal(response.payload.requestId, requestId);
          if (workload === 'photo-import') {
            assert.equal(response.payload.kind, 'photoImportCompleted');
            assert.equal(response.payload.completion.photos.length, 5);
            for (const photo of response.payload.completion.photos) {
              assert.equal(photo.outcome.kind, 'validated');
              assert.equal(photo.outcome.preview.kind, 'prepared');
            }
            const previews = readdirSync(path.join(cacheRoot, 'Media')).filter(file => file.includes(`.${requestId}.`)).sort();
            assert.equal(previews.length, 5);
            // Names depend on the attempt; compare the unordered content hashes instead.
            compareHashes(workload, previews.map(file => fileHash(path.join(cacheRoot, 'Media', file))).sort());
          } else {
            assert.equal(response.payload.kind, 'completed');
            assert.equal(response.payload.completion.sourceCount, 5);
            compareHashes(workload, [fileHash(destination)]);
          }
        }
        samples.push({ workload, variant, round, warmup: round < 2, milliseconds: measured.milliseconds });
        console.log(`${requestId}: ${measured.milliseconds.toFixed(1)} ms${round < 2 ? ' (warmup)' : ''}`);
      }
    }
  }
  report.summary = ['processor-startup', 'photo-import', 'jpeg-render'].map(workload => {
    const medians = Object.fromEntries(['baseline', 'optimized'].map(variant => {
      const values = samples.filter(sample => sample.workload === workload && sample.variant === variant && !sample.warmup)
        .map(sample => sample.milliseconds).sort((a, b) => a - b);
      return [variant, (values[3] + values[4]) / 2];
    }));
    return { workload, medianMs: medians, reductionPercent: 100 * (1 - medians.optimized / medians.baseline) };
  });
  report.outputHashes = Object.fromEntries(expectedHashes);
  report.passed = true;
} finally {
  writeFileSync(path.join(output, 'comparison.json'), `${JSON.stringify(report, null, 2)}\n`);
}
console.log(JSON.stringify(report.summary, null, 2));
