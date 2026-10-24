// P01.02/P01.06 synthetic canvas input journey. No server/profile/media inputs.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
function safeFile(root, request) {
  let pathname;
  try { pathname = decodeURIComponent(new URL(request, 'http://127.0.0.1').pathname); } catch { return null; }
  if (pathname.includes('\\') || pathname.includes('\0')) return null;
  const file = path.resolve(root, '.' + pathname, pathname.endsWith('/') ? 'index.html' : '');
  return file.startsWith(root + path.sep) ? file : null;
}
function summaryPattern(pattern) { return /^\d+ rows/.test(pattern) ? '^' + pattern : pattern; }
async function closeBounded(resource) {
  let timer;
  try {
    await Promise.race([resource.close(),new Promise((_,reject) => { timer=setTimeout(() => reject(new Error('cleanup deadline')),5000); })]);
  } finally { clearTimeout(timer); }
}
async function main() {
  const root = path.resolve(process.argv[2]);
  const output = path.resolve(process.argv[3]);
  // Never replace a previous experiment.
  fs.mkdirSync(output, {recursive:false});
  const engines = require(process.env.P01_PLAYWRIGHT || 'playwright');
  const manifest = {schema:1, synthetic:true, results:[], bundles:{}, sources:{}};
  for (const shell of ['web', 'remote']) for (const file of ['index.html','app.js','app_bg.wasm']) {
    manifest.bundles[shell + '/' + file] = hash(path.join(root, shell, file));
  }
  for (const file of ['tests/browser/input.cjs','crates/browser-spike/src/browser.rs','crates/spike-ui/src/lib.rs','crates/spike-ui/src/text_input.rs','tools/app/browser-shell.html','tools/app/build-web.py','Cargo.lock','Cargo.toml','rust-toolchain.toml','crates/spike-ui/Cargo.toml','crates/browser-spike/Cargo.toml','apps/web/src/lib.rs','apps/remote/src/lib.rs']) {
    const source = path.join(process.env.P01_SOURCE_ROOT || path.resolve(__dirname, '../..'), file);
    if (process.env.P01_SOURCE_ROOT || fs.existsSync(source)) manifest.sources[file] = hash(source);
  }
  const server = http.createServer((req,res) => {
    const file = safeFile(root, req.url);
    if (!file || !fs.existsSync(file) || !fs.statSync(file).isFile()) { res.writeHead(404); res.end(); return; }
    res.setHeader('Content-Type', file.endsWith('.wasm') ? 'application/wasm' : file.endsWith('.js') ? 'text/javascript' : 'text/html');
    fs.createReadStream(file).pipe(res);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const origin = 'http://127.0.0.1:' + server.address().port;
  try {
    for (const engine of ['chromium','webkit','firefox']) for (const profile of [{touch:false,dpr:1},{touch:true,dpr:1},{touch:true,dpr:2}]) for (const shell of ['web','remote']) {
      const {touch,dpr} = profile;
      const result = {engine,touch,dpr,shell,checks:[],result:'failed',stage:'launch'};
      manifest.results.push(result);
      let browser, context, page;
      try {
        browser = await engines[engine].launch({headless:engine !== 'firefox',timeout:20000,...(engine === 'chromium' ? {args:['--no-sandbox']} : {})});
        result.version = browser.version();
        result.stage = 'context';
        context = await browser.newContext({viewport:{width:1100,height:1000},hasTouch:touch,deviceScaleFactor:dpr});
        page = await context.newPage();
        page.setDefaultTimeout(8000);
        let pageErrors = 0;
        page.on('pageerror', () => pageErrors++);
        result.stage = 'startup';
        await page.goto(origin + '/' + shell + '/', {timeout:30000});
        const summary = page.locator('#table-status');
        await summary.waitFor({state:'visible',timeout:30000});
        const expect = async (pattern) => {
          pattern = summaryPattern(pattern);
          await page.waitForFunction(source => new RegExp(source).test(document.querySelector('#table-status').textContent), pattern, {timeout:8000});
          return summary.textContent();
        };
        await expect((shell === 'remote' ? '100' : '1000') + ' rows, 0 selected');
        const canvas = page.locator('#canvas');
        result.geometry = await canvas.evaluate(canvas => new Promise(resolve => {
          const snapshot = {cssWidth:canvas.getBoundingClientRect().width,backingWidth:canvas.width,dpr:devicePixelRatio};
          const timer = setTimeout(() => { observer.disconnect(); resolve({...snapshot,resizeObserver:'timed-out'}); },1000);
          const observer = new ResizeObserver(entries => {
            clearTimeout(timer); observer.disconnect();
            const entry = entries[0];
            resolve({...snapshot,contentBoxWidth:entry.contentBoxSize?.[0]?.inlineSize ?? null,
              devicePixelContentBoxWidth:entry.devicePixelContentBoxSize?.[0]?.inlineSize ?? null});
          });
          observer.observe(canvas);
        }));
        if (dpr === 2 && result.geometry.backingWidth !== result.geometry.cssWidth*dpr) {
          result.result = 'limited';
          result.reason = 'Emulated DPR differs from ResizeObserver device-pixel backing size; coordinate journey runs separately at DPR1';
          result.checks.push('dpr-emulation-backing-size-mismatch');
          continue;
        }
        const activate = async (x,y) => {
          const box = await canvas.boundingBox();
          assert.ok(box && box.width >= 900 && box.height > y);
          if (touch) await page.touchscreen.tap(box.x+x,box.y+y);
          else await page.mouse.click(box.x+x,box.y+y);
        };
        result.stage = 'unicode-filter';
        // Screenshot-derived canvas coordinates at 1100 CSS px, default egui scale.
        await activate(180,54);
        const agent = page.locator('input[type="text"]');
        await agent.waitFor({state:'attached'});
        await page.waitForFunction(() => document.activeElement?.matches('input[type="text"]'));
        await page.keyboard.insertText('Žluťoučký 東京 مرحبا');
        await expect('0 rows, 0 selected');
        const before = await summary.textContent();
        await page.keyboard.press('ArrowDown');
        assert.equal(await summary.textContent(), before);
        result.checks.push('unicode-filter-reaches-egui','text-arrow-does-not-select-table');
        result.stage = 'exact-filter';
        await page.keyboard.press('ControlOrMeta+A');
        await page.keyboard.insertText('Track 000023');
        await expect('1 rows, 0 selected');
        await page.keyboard.press('ControlOrMeta+A');
        await page.keyboard.press('Backspace');
        await expect((shell === 'remote' ? '100' : '1000') + ' rows, 0 selected');
        result.checks.push('exact-filter-and-clear');
        result.stage = 'row-selection';
        await activate(160,225);
        await expect('1 selected');
        result.selection = await summary.textContent();
        assert.match(result.selection,/Track 000000/);
        result.checks.push(touch ? 'touch-row-select' : 'pointer-row-select');
        result.stage = 'table-focus';
        await activate(150,142);
        await page.keyboard.press('End');
        result.end = await expect(shell === 'remote' ? 'Track 000099' : 'Track 000999');
        await page.keyboard.press('Home');
        await expect('Track 000000');
        result.checks.push('focus-control-and-keyboard-home-end');
        result.stage = 'viewport';
        await page.setViewportSize({width:900,height:800});
        await page.waitForFunction(() => document.querySelector('canvas').width === Math.round(900*devicePixelRatio));
        await page.keyboard.press('End');
        await expect(shell === 'remote' ? 'Track 000099' : 'Track 000999');
        result.viewport = await page.evaluate(() => ({width:innerWidth,height:innerHeight,dpr:devicePixelRatio,canvasWidth:document.querySelector('canvas').width}));
        result.checks.push('resize-retains-keyboard-table-input');
        assert.equal(pageErrors,0);
        if (shell === 'remote') assert.equal(await page.locator('#media-controls').isVisible(),false);
        assert.equal(await page.locator('audio').evaluate(a => a.paused && !a.getAttribute('src')),true);
        result.checks.push('no-media-loaded','no-page-errors');
        result.result = 'passed';
      } catch(error) {
        // Preserve stage and error class; never log arbitrary browser or input payloads.
        result.failureType = error.name;
        if (page) {
          result.observedSummary = await page.locator('#table-status').textContent({timeout:1000}).catch(() => null);
          await page.screenshot({path:path.join(output, engine+'-'+touch+'-'+dpr+'-'+shell+'-failure.png'),timeout:3000}).catch(() => {});
        }
        if (result.stage === 'context' && touch && /hasTouch|touch.*support|not supported/i.test(error.message)) {
          result.result = 'not-run'; result.reason = 'Playwright engine rejects touch context option';
        } else process.exitCode = 1;
      } finally {
        for (const resource of [context,browser]) if (resource) {
          try { await closeBounded(resource); } catch { result.cleanupFailure = true; process.exitCode = 1; }
        }
        console.log(JSON.stringify({engine,touch,dpr,shell,result:result.result,stage:result.stage}));
      }
    }
  } finally {
    await new Promise(resolve => server.close(resolve));
    fs.writeFileSync(path.join(output,'results.json'),JSON.stringify(manifest,null,2)+'\n');
  }
  console.log(JSON.stringify(manifest.results.map(({engine,touch,shell,result,stage}) => ({engine,touch,shell,result,stage}))));
}
module.exports = {safeFile,summaryPattern};
if (require.main === module) main().catch(() => { console.error('Input harness failed; no private payloads logged.'); process.exitCode=1; });
