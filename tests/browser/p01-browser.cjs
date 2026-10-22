// Optional isolated browser experiment. No upstream source or live credentials.
// P01_PLAYWRIGHT points to an externally installed Playwright 1.56.1 module.
const { chromium, firefox, webkit } = require(process.env.P01_PLAYWRIGHT || 'playwright');
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const root = path.resolve(process.argv[2]);
const output = path.resolve(process.argv[3]);
const tone = Buffer.alloc(44 + 48000 * 2 * 5);
tone.write('RIFF', 0); tone.writeUInt32LE(tone.length - 8, 4); tone.write('WAVEfmt ', 8);
tone.writeUInt32LE(16, 16); tone.writeUInt16LE(1, 20); tone.writeUInt16LE(1, 22);
tone.writeUInt32LE(48000, 24); tone.writeUInt32LE(96000, 28); tone.writeUInt16LE(2, 32); tone.writeUInt16LE(16, 34);
tone.write('data', 36); tone.writeUInt32LE(tone.length - 44, 40);
for (let i = 0; i < 48000 * 5; i++) tone.writeInt16LE(Math.round(Math.sin(i * Math.PI * 2 * 750 / 48000) * 8192), 44 + i * 2);
const listen = server => new Promise(resolve => server.listen(0, '127.0.0.1', () => resolve(server.address().port)));
const close = server => new Promise(resolve => server.close(resolve));
let origin;
const counts = { authenticated: 0, rejected: 0, cors: 0, codec: 0 };
const audioServer = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://fixture');
  if (url.pathname !== '/cors-denied') res.setHeader('Access-Control-Allow-Origin', origin);
  if (url.searchParams.get('token') !== 'public-disposable-fixture') { counts.rejected++; res.writeHead(403).end(); return; }
  counts.authenticated++;
  if (url.pathname === '/cors-denied') counts.cors++;
  if (url.pathname === '/bad-codec') { counts.codec++; res.writeHead(200, {'Content-Type':'audio/ogg'}).end('invalid-owned-codec-fixture'); return; }
  res.writeHead(200, {'Content-Type':'audio/wav', 'Content-Length':tone.length}); res.end(tone);
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
        stage = 'analysis';
        await page.evaluate(async () => { window.p01Module = await import('./app.js'); window.p01Module.set_analysis_visible(true); });
        const sample = await page.waitForFunction(() => { const peak = window.p01Module.analysis_peak(); return peak > 0.01 ? peak : false; });
        const analysisPeak = await sample.jsonValue();
        assert.ok(analysisPeak > 0.01);
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
        stage = 'remote-shell';
        await page.goto(origin + '/remote/');
        await page.waitForFunction(() => document.querySelector('#status').textContent.includes('Remote presentation experiment'), null, {timeout:30000});
        assert.equal(await page.locator('#media-controls').isVisible(), false);
        await page.screenshot({path:path.join(output, name + '-remote.png')});
        assert.deepEqual(errors, []);
        results.push({browser:name, version:browser.version(), shell:'passed', authenticatedWav:played, authError, codecError, corsError, autoplay, analysisPeak, pageErrors:errors,
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
