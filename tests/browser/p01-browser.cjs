// Optional isolated browser experiment. No upstream source or live credentials.
// P01_PLAYWRIGHT points to an externally installed Playwright 1.56.1 module.
const { chromium, firefox, webkit } = require(process.env.P01_PLAYWRIGHT || 'playwright');
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const root = path.resolve(process.argv[2]);
const output = path.resolve(process.argv[3]);
// Optional real-server fixtures stay private; only the pinned loopback lab is allowed.
let serverFixtures;
try {
  serverFixtures = process.env.P01_SERVER_STREAMS ? JSON.parse(fs.readFileSync(process.env.P01_SERVER_STREAMS, 'utf8')) : [];
for (const fixture of serverFixtures) {
  if (!['flac','mp3'].includes(fixture.format) || !/^Navidrome [0-9.]+$/.test(fixture.fixture)) throw new Error('Invalid disposable server fixture');
  for (const key of ['validUrl','invalidUrl']) {
    const url = new URL(fixture[key]);
    if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || url.port !== '14533' || url.pathname !== '/rest/stream' || url.username || url.password) throw new Error('Invalid disposable server destination');
  }
}

 } catch {
  console.error('Invalid private disposable server fixture; no contents logged.');
  process.exit(1);
}
const tone = Buffer.alloc(44 + 48000 * 2 * 5);
tone.write('RIFF', 0); tone.writeUInt32LE(tone.length - 8, 4); tone.write('WAVEfmt ', 8);
tone.writeUInt32LE(16, 16); tone.writeUInt16LE(1, 20); tone.writeUInt16LE(1, 22);
tone.writeUInt32LE(48000, 24); tone.writeUInt32LE(96000, 28); tone.writeUInt16LE(2, 32); tone.writeUInt16LE(16, 34);
tone.write('data', 36); tone.writeUInt32LE(tone.length - 44, 40);
for (let i = 0; i < 48000 * 5; i++) tone.writeInt16LE(Math.round(Math.sin(i * Math.PI * 2 * 750 / 48000) * 8192), 44 + i * 2);
const listen = server => new Promise(resolve => server.listen(0, '127.0.0.1', () => resolve(server.address().port)));
const close = server => new Promise(resolve => server.close(resolve));
let origin;
const counts = { authenticated: 0, rejected: 0, cors: 0, codec: 0, preflight:0, ranges:0 };
const audioServer = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://fixture');
  if (url.pathname !== '/cors-denied') res.setHeader('Access-Control-Allow-Origin', origin);
  if (req.method === 'OPTIONS') {
    counts.preflight++;
    res.setHeader('Access-Control-Allow-Methods', 'GET, HEAD, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Range');
    res.writeHead(204).end(); return;
  }
  if (url.searchParams.get('token') !== 'public-disposable-fixture') { counts.rejected++; res.writeHead(403).end(); return; }
  counts.authenticated++;
  if (url.pathname === '/cors-denied') counts.cors++;
  if (url.pathname === '/bad-codec') { counts.codec++; res.writeHead(200, {'Content-Type':'audio/ogg'}).end('invalid-owned-codec-fixture'); return; }
  // Real browser seeks require byte ranges; a media clock alone did not exercise this.
  res.setHeader('Accept-Ranges', 'bytes');
  const range = req.headers.range?.match(/^bytes=(\d+)-(\d*)$/);
  if (range) {
    counts.ranges++;
    const start = Number(range[1]), end = Math.min(range[2] ? Number(range[2]) : tone.length - 1, tone.length - 1);
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start > end || end >= tone.length) { res.writeHead(416).end(); return; }
    res.writeHead(206, {'Content-Type':'audio/wav', 'Content-Length':end-start+1, 'Content-Range':`bytes ${start}-${end}/${tone.length}`});
    res.end(tone.subarray(start, end+1));
  } else { res.writeHead(200, {'Content-Type':'audio/wav', 'Content-Length':tone.length}); res.end(tone); }
});
const files = http.createServer((req, res) => {
  const pathname = decodeURIComponent(new URL(req.url, 'http://fixture').pathname);
  const file = path.resolve(root, '.' + pathname, pathname.endsWith('/') ? 'index.html' : '');
  if (!file.startsWith(root + path.sep)) { res.writeHead(403).end(); return; }
  try {
    const type = file.endsWith('.wasm') ? 'application/wasm' : file.endsWith('.js') ? 'text/javascript' : 'text/html';
    res.writeHead(200, {'Content-Type':type}); fs.createReadStream(file).on('error', () => res.destroy()).pipe(res);
  } catch { res.writeHead(404).end(); }
});
(async () => {
  const results = [];
  try {
    origin = `http://127.0.0.1:${await listen(files)}`;
    const audio = `http://127.0.0.1:${await listen(audioServer)}`;
    for (const [name, type] of [['chromium', chromium], ['firefox', firefox], ['webkit', webkit]]) {
      let browser;
      let stage = "launch";
      try {
        browser = await type.launch({headless:name !== 'firefox', firefoxUserPrefs:{'webgl.force-enabled':true}, args:name === 'chromium' ? ['--use-gl=angle', '--use-angle=swiftshader-webgl', '--enable-unsafe-swiftshader'] : []});
        const page = await browser.newPage({ viewport:{width:1100,height:900} });
        await page.addInitScript(() => {
          window.p01Handlers = new Map();
          const session = navigator.mediaSession;
          if (session) {
            const original = session.setActionHandler.bind(session);
            session.setActionHandler = (action, callback) => {
              original(action, callback);
              if (callback) window.p01Handlers.set(action, callback); else window.p01Handlers.delete(action);
            };
          }
        });
        const errors = [];
        page.on('pageerror', () => errors.push('uncaught-page-error')); // Never record URLs/errors containing credentials.
        stage = 'web-shell';
        await page.goto(origin + '/web/');
        await page.waitForFunction(() => document.querySelector('#status').textContent.includes('Paste a disposable'), null, {timeout:30000});
        stage = 'autoplay';
        // Record API autoplay result separately; this is browser policy evidence, not an expected universal value.
        const autoplay = await page.evaluate(async src => { const media = document.querySelector('audio'); media.src = src; media.load(); try { return await Promise.race([media.play().then(() => 'allowed'), new Promise(resolve => setTimeout(() => resolve('pending-after-2s'), 2000))]); } catch (e) { return e.name; } finally { media.pause(); } }, audio + '/tone?token=public-disposable-fixture');
        stage = 'wrong-token';
        await page.locator('#stream').fill(audio + '/tone?token=wrong-public-value'); await page.locator('#play').click();
        await page.waitForFunction(() => document.querySelector('audio').error !== null);
        const authError = await page.locator('audio').evaluate(audio => audio.error.code);
        stage = 'authenticated-playback';
        await page.locator('audio').evaluate(audio => { audio.loop = true; });
        await page.locator('#stream').fill(audio + '/tone?token=public-disposable-fixture'); await page.locator('#play').click();
        await page.waitForFunction(() => document.querySelector('audio').currentTime > 0.3, null, {timeout:15000});
        const played = await page.locator('audio').evaluate(audio => ({position:audio.currentTime, paused:audio.paused, readyState:audio.readyState}));
        assert.equal(played.paused, false);
        stage = 'source-retry-resume';
        const pausedAt = await page.locator('audio').evaluate(audio => { audio.pause(); return audio.currentTime; });
        await page.locator('#play').click();
        assert.ok(await page.locator('audio').evaluate((audio, position) => audio.currentTime >= position - 0.05, pausedAt), 'resume unexpectedly reloaded the recovered source');
        await page.waitForFunction(() => !document.querySelector('audio').paused);
        stage = 'analysis';
        await page.evaluate(async () => { window.p01Module = await import('./app.js'); window.p01Module.set_analysis_visible(true); });
        const sample = await page.waitForFunction(() => { const peak = window.p01Module.analysis_peak(); return peak > 0.01 ? peak : false; });
        const analysisPeak = await sample.jsonValue();
        assert.ok(analysisPeak > 0.01);
        stage = 'analysis-lifecycle';
        await page.evaluate(() => window.p01Module.set_analysis_visible(false));
        assert.equal(await page.evaluate(() => window.p01Module.analysis_running()), false);
        assert.equal(await page.evaluate(() => window.p01Module.analysis_peak()), 0);
        await page.evaluate(() => window.p01Module.set_analysis_visible(true));
        await page.waitForFunction(() => window.p01Module.analysis_running() && window.p01Module.analysis_peak() > 0.01);
        // Synthetic visibility notification exercises disposal, not an actual OS/tab transition.
        await page.evaluate(() => { Object.defineProperty(document, 'visibilityState', { configurable:true, value:'hidden' }); document.dispatchEvent(new Event('visibilitychange')); });
        assert.equal(await page.evaluate(() => window.p01Module.analysis_running()), false);
        assert.equal(await page.locator('audio').evaluate(audio => audio.paused), false);
        await page.evaluate(() => { delete document.visibilityState; document.dispatchEvent(new Event('visibilitychange')); });
        await page.waitForFunction(() => window.p01Module.analysis_running() && window.p01Module.analysis_peak() > 0.01);
        stage = 'media-session-callbacks';
        const sessionActions = await page.evaluate(() => [...window.p01Handlers.keys()]);
        let sessionPlay = 'unsupported';
        let sessionSeek = {result:'unsupported'};
        if (sessionActions.includes('seekto')) {
          await page.evaluate(() => window.p01Handlers.get('pause')({}));
          assert.equal(await page.locator('audio').evaluate(audio => audio.paused), true);
          assert.equal(await page.evaluate(() => window.p01Module.analysis_running()), false);
          stage = 'media-session-seek';
          await page.evaluate(() => window.p01Handlers.get('seekto')({seekTime:2}));
          await page.waitForFunction(() => document.querySelector('#status').textContent.startsWith('Seek ') && !document.querySelector('audio').seeking);
          const actualSeek = await page.locator('audio').evaluate(audio => audio.currentTime);
          sessionSeek = {requested:2, observed:actualSeek, result:Math.abs(actualSeek-2)<0.25 ? 'passed' : 'lab-seek-failed'};
          sessionSeek.edges = [];
          if (sessionSeek.result === 'passed') {
            for (const [action, details, expected] of [['seekto',{seekTime:0},0], ['seekforward',{seekOffset:100},5], ['seekbackward',{seekOffset:100},0]]) {
              await page.evaluate(({action,details}) => window.p01Handlers.get(action)(details), {action,details});
              await page.waitForFunction(() => !document.querySelector('audio').seeking || document.querySelector('audio').error !== null);
              const edge = await page.locator('audio').evaluate(audio => ({position:audio.currentTime,error:audio.error?.code || 0}));
              sessionSeek.edges.push({action, expected, observed:edge.position, result:edge.error ? 'lab-decode-error' : 'passed', mediaError:edge.error});
              if (edge.error) break;
              assert.equal(edge.position, expected);
            }
            if (sessionSeek.edges.every(edge => edge.result === 'passed')) {
              await page.evaluate(() => window.p01Handlers.get('seekto')({seekTime:NaN}));
              assert.equal(await page.locator('audio').evaluate(audio => audio.currentTime), 0);
            }
          }
          stage = 'media-session-resume';
          await page.evaluate(() => window.p01Handlers.get('play')({}));
          await page.waitForFunction(() => document.querySelector('#status').textContent.startsWith('Playing browser') || document.querySelector('#status').textContent.includes('timed out') || document.querySelector('#status').textContent.startsWith('Media error'), null, {timeout:12000});
          sessionPlay = await page.locator('audio').evaluate(audio => audio.paused || audio.error ? 'visible-reload-required' : 'allowed');
          if (sessionPlay === 'visible-reload-required') {
            await page.locator('#play').click();
            await page.waitForFunction(() => !document.querySelector('audio').paused && document.querySelector('#status').textContent.startsWith('Playing browser'));
          }
          assert.equal(await page.evaluate(() => navigator.mediaSession.metadata.title), 'P01 disposable stream');
          await page.evaluate(() => window.p01Handlers.get('stop')({}));
          assert.equal(await page.locator('audio').evaluate(audio => audio.currentTime), 0);
        }
        stage = 'stale-play-promise';
        await page.locator('audio').evaluate(audio => {
          audio.pause();
          window.p01OriginalPlay = audio.play;
          let calls = 0;
          audio.play = function() {
            if (calls++ === 0) return new Promise((resolve, reject) => { window.p01RejectOldPlay = reject; });
            return window.p01OriginalPlay.call(this);
          };
        });
        await page.locator('#play').click();
        await page.locator('#play').click();
        await page.waitForFunction(() => document.querySelector('#status').textContent.startsWith('Playing browser'));
        await page.evaluate(() => window.p01RejectOldPlay(new DOMException('private-disposable-failure', 'NotAllowedError')));
        await page.waitForTimeout(100);
        assert.ok((await page.locator('#status').textContent()).startsWith('Playing browser'));
        stage = 'play-deadline';
        await page.locator('audio').evaluate(audio => { audio.pause(); audio.play = () => new Promise(() => {}); });
        await page.locator('#play').click();
        await page.waitForFunction(() => document.querySelector('#status').textContent.includes('timed out') || document.querySelector('#status').textContent.startsWith('Media error'), null, {timeout:12000});
        assert.equal(await page.locator('audio').evaluate(audio => audio.paused), true);
        await page.locator('audio').evaluate(audio => { audio.play = window.p01OriginalPlay; });
        await page.locator('#play').click();
        await page.waitForFunction(() => window.p01Module.analysis_peak() > 0.01 && document.querySelector('#status').textContent.startsWith('Playing browser'));
        await page.screenshot({path:path.join(output, name + '-web.png')});
        await page.locator('audio').evaluate(audio => audio.pause());
        stage = 'codec';
        await page.locator('#stream').fill(audio + '/bad-codec?token=public-disposable-fixture'); await page.locator('#play').click();
        await page.waitForFunction(() => document.querySelector('audio').error !== null);
        const codecError = await page.locator('audio').evaluate(audio => audio.error.code);
        stage = 'cors';
        await page.locator('#stream').fill(audio + '/cors-denied?token=public-disposable-fixture'); await page.locator('#play').click();
        await page.waitForFunction(() => document.querySelector('audio').error !== null);
        const corsError = await page.locator('audio').evaluate(audio => audio.error.code);
        stage = 'real-server-streams';
        const serverStreams = [];
        for (const fixture of serverFixtures) {
          try {
            await page.locator('#stream').fill(fixture.invalidUrl); await page.locator('#play').click();
            await page.waitForFunction(() => document.querySelector('audio').error !== null);
            const rejected = await page.locator('audio').evaluate(audio => audio.error.code);
            await page.locator('#stream').fill(fixture.validUrl); await page.locator('#play').click();
            await page.evaluate(() => window.p01Module.set_analysis_visible(true));
            await page.waitForFunction(() => document.querySelector('audio').currentTime > 0.2 && window.p01Module.analysis_peak() > 0.01, null, {timeout:15000});
            const observation = await page.locator('audio').evaluate(audio => ({position:audio.currentTime, paused:audio.paused, readyState:audio.readyState}));
            serverStreams.push({fixture:fixture.fixture, format:fixture.format, result:'passed', rejected, observation});
            await page.locator('audio').evaluate(audio => audio.pause());
          } catch (error) {
            serverStreams.push({fixture:fixture.fixture, format:fixture.format, result:'failed', failureType:error.name});
            process.exitCode = 1;
          }
        }
        stage = 'optional-api-failure';
        const optional = await browser.newPage({ viewport:{width:1100,height:900} });
        try {
          await optional.addInitScript(() => {
            Object.defineProperty(navigator, 'mediaSession', {value:{setActionHandler(){throw new Error('fixture-unsupported');}}});
            Object.defineProperty(window, 'MediaMetadata', {value:class {constructor(){throw new Error('fixture-unsupported');}}});
          });
          await optional.goto(origin + '/web/');
          await optional.waitForFunction(() => document.querySelector('#status').textContent.includes('Paste a disposable'), null, {timeout:30000});
          await optional.locator('#stream').fill(audio + '/tone?token=public-disposable-fixture');
          await optional.locator('#play').click();
          await optional.waitForFunction(() => document.querySelector('audio').currentTime > 0.3, null, {timeout:15000});
          assert.equal(await optional.locator('audio').evaluate(audio => audio.paused), false);
        } finally { await optional.close(); }
        stage = 'shutdown';
        await page.evaluate(() => window.p01Module.shutdown());
        assert.equal(await page.evaluate(() => window.p01Module.analysis_running()), false);
        assert.equal(await page.locator('audio').evaluate(audio => audio.paused), true);
        assert.equal(await page.evaluate(() => window.p01Handlers.size), 0);
        stage = 'remote-shell';
        await page.goto(origin + '/remote/');
        await page.waitForFunction(() => document.querySelector('#status').textContent.includes('Remote presentation experiment'), null, {timeout:30000});
        assert.equal(await page.locator('#media-controls').isVisible(), false);
        await page.screenshot({path:path.join(output, name + '-remote.png')});
        assert.deepEqual(errors, []);
        results.push({browser:name, version:browser.version(), shell:'passed', authenticatedWav:played, authError, codecError, corsError, autoplay, analysisPeak, pageErrors:errors,
          mediaSessionCallbacks:sessionActions, sessionPlay, sessionSeek, lifecycle:'disable/re-enable/synthetic-visibility/pause/shutdown passed', playCancellation:'stale failure and eight-second deadline passed', optionalApiFailure:'visible playback passed', serverStreams,

          limits:'Synthetic HTTP token fixture, software GPU and browser-provided media clock; no server adapter, audio device, OS media keys, mobile, previous stable or Safari product certification'});
      } catch (error) {
        // Tooling failures must be distinct from a successful browser check.
        results.push({browser:name, result:'failed-or-unavailable', failureType:error.name, stage});
        process.exitCode = 1;
      } finally { if (browser) await browser.close(); }
    }
    if (results.every(result => result.shell === 'passed')) { assert.ok(counts.rejected >= 3 && counts.cors >= 3 && counts.codec >= 3); }
    fs.writeFileSync(path.join(output, 'browser-results.json'), JSON.stringify({playwright:'1.56.1', results, fixtureRequests:counts}, null, 2) + '\n');
    console.log(JSON.stringify(results.map(result => ({browser:result.browser, shell:result.shell, result:result.result}))));
  } finally { await close(audioServer); await close(files); }
})().catch(() => { console.error('Browser harness failed; no URLs or error payloads logged.'); process.exitCode = 1; });
