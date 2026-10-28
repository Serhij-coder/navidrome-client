// Test-only WASM bundle built from tests/storage/browser_probe.rs; no app profile.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const assert = require('node:assert/strict');
const { chromium, firefox, webkit } = require(process.env.P02_PLAYWRIGHT || 'playwright');
const bundle = path.resolve(process.argv[2]);
const key = 'feishin-rust:dev:p02:profiles:v1';
const server = http.createServer((req, res) => {
  if (req.url === '/') { res.end('<!doctype html><title>P02 owned storage probe</title>'); return; }
  const files = { '/probe.js': ['probe.js', 'text/javascript'], '/probe_bg.wasm': ['probe_bg.wasm', 'application/wasm'] };
  const file = files[req.url];
  if (!file) { res.writeHead(404).end(); return; }
  res.setHeader('Content-Type', file[1]);
  res.end(fs.readFileSync(path.join(bundle, file[0])));
});
async function connect(page, origin) {
  await page.goto(origin);
  await page.evaluate(async () => { const module = await import('/probe.js'); await module.default(); window.probe = module.storage_probe; });
}
(async () => {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  try {
    for (const [name, engine] of Object.entries({ chromium, firefox, webkit })) {
      const browser = await engine.launch({ headless: true, timeout: 30000 });
      try {
        const context = await browser.newContext();
        const page = await context.newPage();
        await connect(page, origin);
        assert.equal(await page.evaluate(() => window.probe('defaults')), true);
        await page.evaluate(() => localStorage.setItem('store_settings', 'original fixture remains'));
        assert.equal(await page.evaluate(() => window.probe('save')), true);
        await connect(page, origin);
        assert.equal(await page.evaluate(() => window.probe('reload')), true);
        assert.equal(await page.evaluate(() => localStorage.getItem('store_settings')), 'original fixture remains');
        // Storage is isolated to a fresh browser profile/context.
        const fresh = await browser.newContext();
        const other = await fresh.newPage();
        await connect(other, origin);
        assert.equal(await other.evaluate(() => window.probe('defaults')), true);
        await fresh.close();
        // Explicit fault injection at the real browser storage API boundary.
        assert.equal(await page.evaluate(key => {
          const before = localStorage.getItem(key);
          const original = Storage.prototype.setItem;
          Storage.prototype.setItem = () => { throw new DOMException('fixture', 'QuotaExceededError'); };
          let rejected = false;
          try { window.probe('save'); } catch { rejected = true; }
          finally { Storage.prototype.setItem = original; }
          return rejected && localStorage.getItem(key) === before;
        }, key), true);
        assert.equal(await page.evaluate(() => {
          const original = Storage.prototype.getItem;
          Storage.prototype.getItem = () => { throw new DOMException('fixture', 'SecurityError'); };
          let rejected = false;
          try { window.probe('defaults'); } catch { rejected = true; }
          finally { Storage.prototype.getItem = original; }
          return rejected;
        }), true);
        for (const raw of ['{"schema_version":2,"data":{}}', '{malformed']) {
          assert.equal(await page.evaluate(({key, raw}) => {
            localStorage.setItem(key, raw);
            let rejected = false;
            try { window.probe('reject-save'); } catch { rejected = true; }
            return rejected && localStorage.getItem(key) === raw;
          }, {key, raw}), true);
        }
        assert.equal(await page.evaluate(key => {
          const raw = '{"schema_version":1,"data":{"preferences":{"language":12}}}';
          localStorage.setItem(key, raw);
          return window.probe('partial') && localStorage.getItem(key) === raw;
        }, key), true);
        await context.close();
        console.log(`${name} ${browser.version()}: 10 storage checks passed`);
      } finally { await browser.close(); }
    }
  } finally { await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error.name + ': ' + error.message); process.exitCode = 1; });
