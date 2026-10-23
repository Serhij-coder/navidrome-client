// Real browser seeking/seeked events through the native audio-element path.
const assert = require('node:assert/strict');

async function pauseAt(page, observeSeek, target = 1) {
  await page.locator('audio').evaluate(audio => audio.pause());
  assert.equal((await observeSeek(page, {target, action:'seekto', details:{seekTime:target}})).result, 'passed');
}
async function nativeSeek(page, target) {
  return page.locator('audio').evaluate((audio, target) => new Promise(resolve => {
    let timer;
    const finish = result => {
      clearTimeout(timer);
      audio.removeEventListener('seeked', settled);
      resolve(result);
    };
    const settled = () => finish({completion:'seeked', position:audio.currentTime});
    audio.addEventListener('seeked', settled, {once:true});
    timer = setTimeout(() => finish({completion:'timeout', position:audio.currentTime}), 3000);
    audio.currentTime = target;
  }), target);
}
async function nativeSeekLifecycle(page, source, observeSeek) {
  // The preceding journey intentionally leaves this page on a CORS failure.
  await page.locator('#stream').fill(source);
  await page.locator('#play').click();
  await page.waitForFunction(() => !document.querySelector('audio').paused && document.querySelector('#status').textContent.startsWith('Playing browser'), null, {timeout:12000});
  await pauseAt(page, observeSeek);
  const direct = await nativeSeek(page, 2);
  assert.equal(direct.completion, 'seeked');
  assert.ok(Math.abs(direct.position - 2) < 0.25);
  assert.equal(await page.locator('#status').textContent(), 'Seek completed in browser stream.');

  // Hold a callback request without an event, then let a real element seek
  // supersede it. The adapter must adopt the new target rather than fail it.
  await page.locator('audio').evaluate(audio => {
    const position = audio.currentTime;
    Object.defineProperty(audio, 'currentTime', {configurable:true, get:() => position, set() {}});
  });
  try {
    await page.evaluate(() => window.p01Handlers.get('seekto')({seekTime:4}));
    assert.equal(await page.locator('#status').textContent(), 'Seeking browser stream.');
  } finally { await page.locator('audio').evaluate(audio => { delete audio.currentTime; }); }
  const replaced = await nativeSeek(page, 3);
  assert.equal(replaced.completion, 'seeked');
  assert.ok(Math.abs(replaced.position - 3) < 0.25);
  if (await page.locator('#status').textContent() !== 'Seek completed in browser stream.') {
    const error = new Error();
    error.name = 'NativeSeekReplacementFailure';
    throw error;
  }

  // A real browser seek starts and settles, but its completion is deliberately
  // withheld from the adapter. No seeking/seeked event is fabricated.
  await page.locator('audio').evaluate(audio => {
    window.p01SeekedWithheld = false;
    window.p01WithholdSeeked = event => { window.p01SeekedWithheld = true; event.stopImmediatePropagation(); };
    audio.addEventListener('seeked', window.p01WithholdSeeked, true);
    audio.currentTime = 5;
  });
  try {
    await page.waitForFunction(() => document.querySelector('#status').textContent === 'Seeking browser stream.', null, {timeout:3000});
    await page.waitForFunction(() => window.p01SeekedWithheld, null, {timeout:3000});
    await page.waitForFunction(() => document.querySelector('#status').textContent.startsWith('Seek timed out'), null, {timeout:10000});
    assert.equal(await page.locator('audio').evaluate(audio => audio.paused), true);
    assert.equal(await page.evaluate(() => window.p01Module.analysis_running()), false);
  } finally {
    await page.locator('audio').evaluate(audio => {
      audio.removeEventListener('seeked', window.p01WithholdSeeked, true);
      delete window.p01WithholdSeeked;
      delete window.p01SeekedWithheld;
    });
  }
  await page.locator('#stream').fill(source);
  await page.locator('#play').click();
  await page.waitForFunction(() => !document.querySelector('audio').paused && document.querySelector('#status').textContent.startsWith('Playing browser'), null, {timeout:12000});
  await pauseAt(page, observeSeek);
  // Replace an observed element seek with a callback request.
  await page.locator('audio').evaluate(audio => {
    window.p01SeekedWithheld = false;
    window.p01WithholdSeeked = event => { window.p01SeekedWithheld = true; event.stopImmediatePropagation(); };
    audio.addEventListener('seeked', window.p01WithholdSeeked, true);
    audio.currentTime = 2;
  });
  try {
    await page.waitForFunction(() => document.querySelector('#status').textContent === 'Seeking browser stream.', null, {timeout:3000});
    await page.waitForFunction(() => window.p01SeekedWithheld, null, {timeout:3000});
  } finally {
    await page.locator('audio').evaluate(audio => {
      audio.removeEventListener('seeked', window.p01WithholdSeeked, true);
      delete window.p01WithholdSeeked;
      delete window.p01SeekedWithheld;
    });
  }
  assert.equal((await observeSeek(page, {target:4, action:'seekto', details:{seekTime:4}})).result, 'passed');
  assert.equal(await page.locator('#status').textContent(), 'Seek completed in browser stream.');
  // Source replacement cancels another native request before its deadline.
  await pendingNativeShutdownSeek(page);
  await page.locator('#stream').fill(source + '&case=owned-native-seek-cancel');
  await page.locator('#play').click();
  await page.waitForFunction(() => !document.querySelector('audio').paused && document.querySelector('#status').textContent.startsWith('Playing browser'), null, {timeout:12000});
  await pauseAt(page, observeSeek);
  await page.waitForTimeout(8500);
  assert.equal(await page.locator('#status').textContent(), 'Seek completed in browser stream.');
  // Completion must still match the latest observed target. Override only its
  // reported position at the real completion to exercise this failure path.
  await page.locator('audio').evaluate(audio => {
    audio.addEventListener('seeked', () => {
      const wrong = audio.currentTime + 1;
      Object.defineProperty(audio, 'currentTime', {configurable:true, get:() => wrong});
    }, {capture:true, once:true});
  });
  try {
    await nativeSeek(page, 7);
    assert.ok((await page.locator('#status').textContent()).startsWith('Seek did not reach'));
    assert.equal(await page.locator('audio').evaluate(audio => audio.paused), true);
  } finally { await page.locator('audio').evaluate(audio => { delete audio.currentTime; }); }
  await page.locator('#play').click();
  await page.waitForFunction(() => !document.querySelector('audio').paused && document.querySelector('#status').textContent.startsWith('Playing browser'), null, {timeout:12000});
  return {elementSeek:'passed', replacesCallback:'passed', missingCompletion:'passed', visibleRecovery:'passed', cancelledDeadline:'passed', callbackReplacesElement:'passed', sourceChange:'passed', mismatch:'passed'};
}
async function pendingNativeShutdownSeek(page) {
  await page.locator('audio').evaluate(audio => {
    audio.pause();
    window.p01SeekedWithheld = false;
    window.p01WithholdSeeked = event => { window.p01SeekedWithheld = true; event.stopImmediatePropagation(); };
    audio.addEventListener('seeked', window.p01WithholdSeeked, true);
    audio.currentTime = 2;
  });
  try {
    await page.waitForFunction(() => document.querySelector('#status').textContent === 'Seeking browser stream.', null, {timeout:3000});
    await page.waitForFunction(() => window.p01SeekedWithheld, null, {timeout:3000});
  } finally {
    await page.locator('audio').evaluate(audio => {
      audio.removeEventListener('seeked', window.p01WithholdSeeked, true);
      delete window.p01WithholdSeeked;
      delete window.p01SeekedWithheld;
    });
  }
}
module.exports = {nativeSeekLifecycle, pendingNativeShutdownSeek};
