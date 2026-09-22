import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHeadlessBrowserSession } from './HeadlessBrowserSession.mjs';
import { webdriverElementId } from './UiAcceptance.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.resolve(process.env.MYALBUNS_FAVORITE_OUTPUT ?? path.join(root, '.scratch/favorite-visibility'));
mkdirSync(output, { recursive: true });
const browser = createHeadlessBrowserSession({ root, output, windowSize: '1000,800' });
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));

try {
  const { port, request, session } = await browser.start();
  const endpoint = `/session/${session}`;
  const execute = async script => request('POST', `${endpoint}/execute/sync`, { script, args: [] });
  const move = async (x, y) => request('POST', `${endpoint}/actions`, { actions: [{
    type: 'pointer', id: 'mouse', parameters: { pointerType: 'mouse' },
    actions: [{ type: 'pointerMove', duration: 0, origin: 'viewport', x, y }],
  }] });
  const key = async value => request('POST', `${endpoint}/actions`, { actions: [{
    type: 'key', id: 'keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }],
  }] });
  const snapshot = async id => execute(`
    const card = document.querySelector('.global-project-card[data-project-id="${id}"]');
    const star = card?.querySelector('.global-project-favorite');
    return star && { pressed: star.getAttribute('aria-pressed'), opacity: getComputedStyle(star).opacity,
      focused: document.activeElement === star, focusVisible: star.matches(':focus-visible'), hovered: card.matches(':hover'),
      rect: { x: star.getBoundingClientRect().x, y: star.getBoundingClientRect().y,
        width: star.getBoundingClientRect().width, height: star.getBoundingClientRect().height } };
  `);
  const waitState = async (id, pressed) => {
    for (let i = 0; i < 80; i++) {
      const state = await snapshot(id);
      if (state?.pressed === pressed) { await wait(160); return snapshot(id); }
      await wait(50);
    }
    throw new Error(`Timed out waiting for ${id} pressed=${pressed}`);
  };
  const capture = async name => writeFileSync(path.join(output, `${name}.png`),
    Buffer.from(await request('GET', `${endpoint}/screenshot`), 'base64'));
  await request('POST', `${endpoint}/url`, { url: `http://127.0.0.1:${port}/welcome-preview.html?recents=favorites` });
  await waitState('p2', 'false');
  await move(2, 2);
  const initial = await snapshot('p2');
  assert.equal(initial.opacity, '0', `initial nonfavorite: ${JSON.stringify(initial)}`);
  const center = state => ({ x: Math.round(state.rect.x + state.rect.width / 2), y: Math.round(state.rect.y + state.rect.height / 2) });
  const p = center(initial);
  await move(p.x, p.y);
  await request('POST', `${endpoint}/element/${webdriverElementId(await request('POST', `${endpoint}/element`, {
    using: 'css selector', value: '.global-project-card[data-project-id="p2"] .global-project-favorite',
  }))}/click`, {});
  const marked = await waitState('p2', 'true');
  assert.equal(marked.hovered, false, 'moved favorite no longer occupies the old pointer location');
  assert.equal(marked.opacity, '1', 'favorite remains visible after leaving its old position');
  await move(2, 2);
  assert.equal(marked.focused, true, 'focus follows moved favorite card');
  assert.equal((await snapshot('p2')).opacity, '1', 'favorite remains visible away from hover');
  await capture('favorite-pressed-outside');
  const moved = await snapshot('p2');
  const q = center(moved);
  await move(q.x, q.y);
  await request('POST', `${endpoint}/element/${webdriverElementId(await request('POST', `${endpoint}/element`, {
    using: 'css selector', value: '.global-project-card[data-project-id="p2"] .global-project-favorite',
  }))}/click`, {});
  const movedBack = await waitState('p2', 'false');
  assert.equal(movedBack.hovered, false, 'unfavorited card no longer occupies the old pointer location');
  assert.equal(movedBack.opacity, '0', 'nonfavorite hides even while pointer stays over the old card position');
  await capture('nonfavorite-pointer-old-position');
  await move(2, 2);
  await wait(180);
  const unmarked = await snapshot('p2');
  await capture('nonfavorite-pointer-outside');
  console.log(JSON.stringify({ initial, marked, unmarked }, null, 2));
  assert.equal(unmarked.focused, true, 'focus follows moved nonfavorite card');
  assert.equal(unmarked.hovered, false, 'pointer is outside card');
  assert.equal(unmarked.opacity, '0', 'nonfavorite disappears after mouse leaves');
  await key('\uE004'); // Tab establishes keyboard modality.
  await execute('document.querySelector(\'.global-project-card[data-project-id="p2"] .global-project-launch\').focus();');
  await wait(160);
  assert.equal((await snapshot('p2')).opacity, '1', 'keyboard focus on card reveals its star');
  await key('\uE004');
  await wait(160);
  const keyboard = await snapshot('p2');
  assert.equal(keyboard.focusVisible, true, 'keyboard focus is visible');
  assert.equal(keyboard.opacity, '1', 'nonfavorite remains visible for keyboard focus');
  await key('\uE007'); // Enter
  await waitState('p2', 'true');
  await move(2, 2);
  await key('\uE007');
  await waitState('p2', 'false');
  await move(2, 2);
  const keyboardRestored = await snapshot('p2');
  await capture('nonfavorite-keyboard-focus');
  assert.equal(keyboardRestored.focusVisible, true, 'keyboard focus survives group changes');
  assert.equal(keyboardRestored.opacity, '1', 'keyboard user can see the restored star');
} finally {
  assert.equal(await browser.close(), true, 'owned browser processes close');
}
