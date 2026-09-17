import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHeadlessBrowserSession } from './HeadlessBrowserSession.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.resolve(process.argv[2] ?? path.join(root, '.scratch/ui-acceptance/photo-placement'));
mkdirSync(output, { recursive: true });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const evidence = { gate: 'photo-placement-rendering', collectedAtUtc: new Date().toISOString(),
  passed: false, cleanupCompleted: false, samples: [] };
const browser = createHeadlessBrowserSession({ root, output, windowSize: '1100,800', requestTimeoutMilliseconds: 10000 });
let request, session;
try {
  const started = await browser.start();
  ({ request, session } = started);
  const port = started.port;
  evidence.browserVersion = started.edge.edgeVersion;

  await request('POST', `/session/${session}/url`, { url: `http://127.0.0.1:${port}/photo-placement-preview.html` });
  const execute = script => request('POST', `/session/${session}/execute/sync`, { script, args: [] });
  let ready = false;
  for (let i = 0; i < 200 && !ready; i++) {
    ready = await execute("return document.body.dataset.ready === 'true'");
    if (!ready) await delay(100);
  }
  assert.ok(ready, 'Photo placement fixture must finish');
  evidence.samples = await execute('return window.photoPlacementTest.samples');
  evidence.svgSample = await execute('return window.photoPlacementTest.svgSample');
  evidence.openingSample = await execute('return window.photoPlacementTest.openingSample');
  evidence.replacementSample = await execute('return window.photoPlacementTest.replacementSample');
  evidence.thumbnailSample = await execute('return window.photoPlacementTest.thumbnailSample');
  writeFileSync(path.join(output, 'rendered.png'), Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'));
  assert.equal(evidence.samples.length, 12);
  for (const sample of evidence.samples) {
    assert.deepEqual(sample.first, sample.expected, `Photo ${sample.index + 1} must have image pixels in its first render`);
    assert.deepEqual(sample.settled, sample.first, `Photo ${sample.index + 1} must not change from fallback to image`);
  }
  assert.deepEqual(evidence.svgSample.actual, evidence.svgSample.expected, 'SVG previews must retain rasterized image pixels');
  assert.deepEqual(evidence.openingSample.beforeUrl, [0, 0, 0, 0], 'Opening must not show synthetic artwork before the preview URL arrives');
  assert.deepEqual(evidence.openingSample.beforeTexture, [0, 0, 0, 0], 'Opening must not show synthetic artwork while the texture loads');
  assert.deepEqual(evidence.openingSample.ready, evidence.openingSample.expected, 'Opening must display the real preview when it loads');
  assert.deepEqual(evidence.replacementSample.pending, evidence.replacementSample.before, 'Replacing media must retain the previous pixels while its new texture loads');
  assert.deepEqual(evidence.replacementSample.ready, evidence.replacementSample.expected, 'Replacing media must eventually present the new pixels');
  assert.deepEqual(evidence.thumbnailSample.pending, evidence.thumbnailSample.before, 'The thumbnail must retain the previous pixels while the new image loads');
  assert.deepEqual(evidence.thumbnailSample.ready, evidence.thumbnailSample.expected, 'The thumbnail must present the loaded replacement');
  console.log('PASS: 12 additions show the loaded photo on the first render, with identical settled pixels.');
  console.log('PASS: project opening stays free of synthetic photo pixels until the real preview loads.');
  console.log('PASS: thumbnail and Canvas retain their previous pixels until each replacement is ready.');
} catch (error) {
  evidence.error = error instanceof Error ? error.stack : String(error);
  console.error(evidence.error);
} finally {
  evidence.cleanupCompleted = await browser.close();
  evidence.passed = !evidence.error && evidence.cleanupCompleted;
  writeFileSync(path.join(output, 'evidence.json'), JSON.stringify(evidence, null, 2) + '\n');
  if (!evidence.passed) process.exitCode = 1;
}
