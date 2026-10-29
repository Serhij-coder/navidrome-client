// Owned active-state adoption fixture. No live Feishin profile or media is opened.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const assert = require('node:assert/strict');
const { chromium, firefox, webkit } = require(process.env.P02_PLAYWRIGHT || 'playwright');
const bundle = path.resolve(process.argv[2]);
const activeKey = 'feishin-rust:dev:p02:state:v1';
const stageKey = 'feishin-rust:dev:p02:imports:v1';
const profileKey = 'feishin-rust:dev:p02:profiles:v1';
const server = http.createServer((req, res) => {
  if (req.url === '/') { res.end('<!doctype html><title>P02 owned active state</title>'); return; }
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
        await check('state-defaults');
        assert.equal(await page.evaluate(key => localStorage.getItem(key), activeKey), null); checks++;
        await check('state-adopt');
        const stage = await page.evaluate(key => localStorage.getItem(key), stageKey);
        await connect(page, origin);
        await check('state-reload');
        await check('state-idempotent');
        await check('state-undo-first');
        await connect(page, origin);
        await check('state-redo');
        await check('state-save');
        await check('state-stale-target');
        await check('state-rollback');
        assert.equal(await page.evaluate(key => localStorage.getItem(key), stageKey), stage); checks++;
        await check('state-stale-stage');
        assert.equal(await page.evaluate(key => localStorage.getItem(key), profileKey), legacy); checks++;
        assert.equal(await page.evaluate(() => localStorage.getItem('store_settings')), 'original remains'); checks++;
        assert.equal(await page.evaluate(key => {
          const text = localStorage.getItem(key);
          return !text.includes('SYNTHETIC-SECRET') && !text.includes('SYNTHETIC-TOKEN');
        }, activeKey), true); checks++;
        assert.equal(await page.evaluate(key => {
          const before = localStorage.getItem(key);
          const original = Storage.prototype.setItem;
          Storage.prototype.setItem = () => { throw new DOMException('fixture', 'QuotaExceededError'); };
          let rejected = false;
          try { window.probe('state-save'); } catch { rejected = true; }
          finally { Storage.prototype.setItem = original; }
          return rejected && localStorage.getItem(key) === before;
        }, activeKey), true); checks++;
        for (const damaged of ['{bad', '{"schema_version":999}']) {
          assert.equal(await page.evaluate(({ key, damaged }) => {
            localStorage.setItem(key, damaged);
            let rejected = false;
            try { window.probe('state-save'); } catch { rejected = true; }
            return rejected && localStorage.getItem(key) === damaged;
          }, { key: activeKey, damaged }), true); checks++;
        }
        await context.close();
        console.log(`${name} ${browser.version()}: ${checks} active-state checks passed`);
      } finally { await browser.close(); }
    }
  } finally { await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error.name + ': ' + error.message); process.exitCode = 1; });
