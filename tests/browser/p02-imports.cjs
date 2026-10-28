// Owned import-journal fixture; no original profile/export is opened by the browser.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const assert = require('node:assert/strict');
const { chromium, firefox, webkit } = require(process.env.P02_PLAYWRIGHT || 'playwright');
const bundle = path.resolve(process.argv[2]);
const key = 'feishin-rust:dev:p02:imports:v1';
const server = http.createServer((req, res) => {
  if (req.url === '/') { res.end('<!doctype html><title>P02 owned import journal</title>'); return; }
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
        await page.evaluate(() => {
          localStorage.setItem('store_settings', 'original copy remains');
          localStorage.setItem('feishin-rust:dev:p02:profiles:v1', 'independent development profile remains');
        });
        assert.equal(await page.evaluate(() => window.probe('import-defaults')), true);
        assert.equal(await page.evaluate(() => window.probe('import-save')), true);
        await connect(page, origin);
        assert.equal(await page.evaluate(() => window.probe('import-reload')), true);
        assert.equal(await page.evaluate(() => window.probe('import-rollback')), true);
        await connect(page, origin);
        assert.equal(await page.evaluate(() => window.probe('import-defaults')), true);
        assert.equal(await page.evaluate(() => window.probe('import-redo')), true);
        assert.equal(await page.evaluate(() => window.probe('import-stale')), true);
        assert.equal(await page.evaluate(() => window.probe('import-corrupt-current')), true);
        assert.equal(await page.evaluate(() => localStorage.getItem('store_settings')), 'original copy remains');
        assert.equal(await page.evaluate(() => localStorage.getItem('feishin-rust:dev:p02:profiles:v1')), 'independent development profile remains');
        assert.equal(await page.evaluate(key => {
          const before = localStorage.getItem(key);
          const original = Storage.prototype.setItem;
          Storage.prototype.setItem = () => { throw new DOMException('fixture', 'QuotaExceededError'); };
          let rejected = false;
          try { window.probe('import-rollback'); } catch { rejected = true; }
          finally { Storage.prototype.setItem = original; }
          return rejected && localStorage.getItem(key) === before;
        }, key), true);
        assert.equal(await page.evaluate(key => {
          const before = '{"schema_version":999,"current":null}';
          localStorage.setItem(key, before);
          let rejected = false;
          try { window.probe('import-save'); } catch { rejected = true; }
          return rejected && localStorage.getItem(key) === before;
        }, key), true);
        await context.close();
        console.log(`${name} ${browser.version()}: 12 import-journal checks passed`);
      } finally { await browser.close(); }
    }
  } finally { await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error.name + ': ' + error.message); process.exitCode = 1; });
