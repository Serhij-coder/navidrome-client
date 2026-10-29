// Portable cache lifecycle in a real WASM runtime; no transport/cache persistence.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const assert = require('node:assert/strict');
const { chromium, firefox, webkit } = require(process.env.P02_PLAYWRIGHT || 'playwright');
const bundle = path.resolve(process.argv[2]);
const server = http.createServer((req, res) => {
  if (req.url === '/') { res.end('<!doctype html><title>P02 query cache</title>'); return; }
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
        await page.evaluate(() => { localStorage.setItem('fixture', 'untouched'); sessionStorage.setItem('fixture', 'untouched'); });
        const snapshot = () => page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }));
        const before = await snapshot();
        assert.equal(await page.evaluate(() => window.probe('query-cache-flow')), true);
        assert.equal(await snapshot(), before);
        await connect(page, origin);
        assert.equal(await page.evaluate(() => window.probe('query-cache-flow')), true);
        assert.equal(await snapshot(), before);
        await context.close();
        console.log(`${name} ${browser.version()}: 4 query-cache runtime checks passed`);
      } finally { await browser.close(); }
    }
  } finally { await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error.name + ': ' + error.message); process.exitCode = 1; });
