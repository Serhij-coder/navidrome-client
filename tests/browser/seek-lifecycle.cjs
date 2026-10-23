// Optional real-browser seek lifecycle regressions for the synthetic P01 shell.
const assert = require('node:assert/strict');

async function recover(page, source) {
  await page.locator('#stream').fill(source);
  await page.locator('#play').click();
  await page.waitForFunction(() => !document.querySelector('audio').paused &&
    document.querySelector('#status').textContent.startsWith('Playing browser') &&
    window.p01Module.analysis_peak() > 0.01, null, {timeout:12000});
}
async function holdPosition(page, observeSeek) {
  await page.locator('audio').evaluate(audio => audio.pause());
  const result = await observeSeek(page, {target:1, action:'seekto', details:{seekTime:1}});
  assert.equal(result.result, 'passed');
}
async function blockSetter(page, reject) {
  await page.locator('audio').evaluate((audio, reject) => {
    const position = audio.currentTime;
    Object.defineProperty(audio, 'currentTime', {configurable:true, get:() => position,
      set() { if (reject) throw new DOMException('private-disposable-seek-error', 'InvalidStateError'); }});
  }, reject);
}
async function restoreSetter(page) {
  await page.locator('audio').evaluate(audio => { delete audio.currentTime; });
}
async function seekLifecycle(page, source, observeSeek) {
  // Position fault injection is explicit: one throwing setter, then a setter
  // that accepts requests but never starts a seek or emits a completion event.
  await holdPosition(page, observeSeek);
  try {
    await blockSetter(page, true);
    await page.evaluate(() => window.p01Handlers.get('seekto')({seekTime:2}));
    assert.ok((await page.locator('#status').textContent()).startsWith('Seek rejected'));
    assert.equal(await page.locator('audio').evaluate(audio => audio.paused), true);
  } finally { await restoreSetter(page); }
  await recover(page, source);
  const noOp = await page.evaluate(() => {
    const audio = document.querySelector('audio');
    audio.pause();
    window.p01Handlers.get('seekto')({seekTime:audio.currentTime});
    return document.querySelector('#status').textContent;
  });
  assert.equal(noOp, 'Seek completed in browser stream.');

  await holdPosition(page, observeSeek);
  try {
    await blockSetter(page, false);
    await page.evaluate(() => window.p01Handlers.get('seekto')({seekTime:2}));
    assert.equal(await page.locator('#status').textContent(), 'Seeking browser stream.');
    await page.waitForFunction(() => document.querySelector('#status').textContent.startsWith('Seek timed out'), null, {timeout:10000});
    assert.equal(await page.locator('audio').evaluate(audio => audio.paused), true);
    assert.equal(await page.evaluate(() => window.p01Module.analysis_running()), false);
    await page.evaluate(() => window.p01Handlers.get('play')({}));
    assert.equal(await page.locator('#status').textContent(), 'Playback requires a visible reload after a failed operation.');
    assert.equal(await page.locator('audio').evaluate(audio => audio.paused), true);
    // Native controls call the media element directly, bypassing the callback.
    await page.locator('audio').evaluate(async audio => {
      let timer;
      try {
        await Promise.race([audio.play().catch(() => {}), new Promise(resolve => { timer = setTimeout(resolve, 3000); })]);
      } finally { clearTimeout(timer); }
    });
    await page.waitForFunction(() => document.querySelector('audio').paused &&
      document.querySelector('#status').textContent.startsWith('Playback requires a visible reload'), null, {timeout:3000}).catch(() => {
      const failure = new Error();
      failure.name = 'NativeResumeFailure';
      throw failure;
    });
    assert.equal(await page.evaluate(() => window.p01Module.analysis_running()), false);
  } finally { await restoreSetter(page); }
  await recover(page, source);

  await holdPosition(page, observeSeek);
  try {
    await blockSetter(page, false);
    await page.evaluate(() => window.p01Handlers.get('seekto')({seekTime:2}));
  } finally { await restoreSetter(page); }
  const replacement = await observeSeek(page, {target:3, action:'seekto', details:{seekTime:3}});
  assert.equal(replacement.result, 'passed');
  assert.equal(await page.locator('#status').textContent(), 'Seek completed in browser stream.');

  try {
    await blockSetter(page, false);
    await page.evaluate(() => window.p01Handlers.get('seekto')({seekTime:4}));
  } finally { await restoreSetter(page); }
  // A different synthetic URL forces source replacement through the actual UI.
  await recover(page, source + '&case=owned-seek-cancel');
  await holdPosition(page, observeSeek);
  // Wait beyond both cancelled deadlines using the real browser clock.
  await page.waitForTimeout(8500);
  assert.equal(await page.locator('#status').textContent(), 'Seek completed in browser stream.');
  assert.equal(await page.locator('audio').evaluate(audio => audio.currentTime), 1);
  await recover(page, source);
  return {setterRejection:'passed', samePosition:'passed', timeout:'passed',
    replacement:'passed', sourceChange:'passed', recovery:'passed', failedResume:'passed', nativeResume:'passed'};
}
async function pendingShutdownSeek(page, source, observeSeek) {
  await recover(page, source);
  await holdPosition(page, observeSeek);
  try {
    await blockSetter(page, false);
    await page.evaluate(() => window.p01Handlers.get('seekto')({seekTime:2}));
    assert.equal(await page.locator('#status').textContent(), 'Seeking browser stream.');
  } finally { await restoreSetter(page); }
}
module.exports = {seekLifecycle, pendingShutdownSeek};
