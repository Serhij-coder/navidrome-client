// Real browser local dispatcher/storage fixture. No Web Worker is started.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const assert = require('node:assert/strict');
const { chromium, firefox, webkit } = require(process.env.P02_PLAYWRIGHT || 'playwright');
const bundle = path.resolve(process.argv[2]);
const activeKey = 'feishin-rust:dev:p02:state:v1';
const profileKey = 'feishin-rust:dev:p02:profiles:v1';
const server = http.createServer((req, res) => {
  if (req.url === '/') { res.end('<!doctype html><title>P02 local storage session</title>'); return; }
  const file = { '/probe.js': ['probe.js', 'text/javascript'], '/probe_bg.wasm': ['probe_bg.wasm', 'application/wasm'] }[req.url];
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
        const legacy = '{"schema_version":1,"data":{"preferences":{"language":"de"}}}';
        await page.evaluate(({ profileKey, legacy }) => {
          localStorage.setItem(profileKey, legacy);
          localStorage.setItem('store_settings', 'original remains');
        }, { profileKey, legacy });
        let checks = 0;
        async function check(action) {
          assert.equal(await page.evaluate(action => window.probe(action), action), true, action);
          checks++;
        }
        await check('session-flow');
        await connect(page, origin);
        await check('session-reload');
        const before = await page.evaluate(key => localStorage.getItem(key), activeKey);
        await check('session-controls');
        await check('session-bounds');
        await check('session-discard');
        assert.equal(await page.evaluate(() => {
          const original = Storage.prototype.setItem;
          Storage.prototype.setItem = () => { throw new DOMException('fixture', 'QuotaExceededError'); };
          try { return window.probe('session-write-error'); }
          finally { Storage.prototype.setItem = original; }
        }), true); checks++;
        assert.equal(await page.evaluate(key => localStorage.getItem(key), activeKey), before); checks++;
        assert.equal(await page.evaluate(key => localStorage.getItem(key), profileKey), legacy); checks++;
        assert.equal(await page.evaluate(() => localStorage.getItem('store_settings')), 'original remains'); checks++;
        assert.equal(before.includes('SYNTHETIC-SECRET') || before.includes('SYNTHETIC-TOKEN'), false); checks++;
        await context.close();
        console.log(`${name} ${browser.version()}: ${checks} local-session checks passed`);
      } finally { await browser.close(); }
    }
  } finally { await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error.name + ': ' + error.message); process.exitCode = 1; });
