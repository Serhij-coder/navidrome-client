#!/usr/bin/env node
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { COMMIT, sha256, redact, learnSecrets, verifySource, verifyOutputSeparation } = require('../../tools/app/p00-browser-evidence.cjs');

async function capture({ playwright, electronBinary, runtimeRoot, sourceRoot, auditFile, credentialsFile, privateRoot, outputRoot }) {
    verifyOutputSeparation(privateRoot, outputRoot, credentialsFile);
    const auditBytes = fs.readFileSync(auditFile);
    verifySource(sourceRoot, JSON.parse(auditBytes));
    fs.mkdirSync(privateRoot, { recursive: true, mode: 0o700 });
    fs.mkdirSync(outputRoot, { recursive: true });
    const credentials = JSON.parse(fs.readFileSync(credentialsFile));
    const secrets = new Set([credentials.password, credentials.reader_password]);
    const pw = require(playwright);
    const report = { schema: 1, tasks: ['P00.03', 'P00.06'], commit: COMMIT, sourceAuditSha256: sha256(auditBytes),
        evidenceKind: 'original-desktop-xvfb-and-live-remote-run', date: new Date().toISOString(), screenshots: [], operations: [], socketEvents: [],
        environment: { playwright: require(path.join(playwright, 'package.json')).version, display: 'Xvfb 1440x1000x24', nativeGpu: false, browserSandbox: false },
        instrumentation: ['External launcher retains original package name/version and loads unchanged compiled main/preload/renderer.',
            'Separate profile, updates disabled, analytics opt-out preload and renderer network allowlist before first HTML script.',
            'HTTP listen host forced to 127.0.0.1; local remote service uses port 18433. No LAN remote/device claim.',
            'Virtual display and --disable-gpu/--no-sandbox are characterization conditions, not packaging/performance results.'],
        limitations: ['No AppImage/installer, real desktop compositor, GPU, audio device, keyring or OS media/tray behavior verified.',
            'Remote evidence covers one authenticated client, initial queue, library view and acknowledged clear only.',
            'Runtime wrappers/profiles and raw credentials are private external reference inputs, not project dependencies.'] };
    const pkg = JSON.parse(fs.readFileSync(path.join(runtimeRoot, 'package.json')));
    const guard = path.join(privateRoot, 'guard.cjs');
    const preload = path.join(privateRoot, 'analytics-preload.cjs');
    const launcher = path.join(privateRoot, 'launcher');
    fs.mkdirSync(launcher, { recursive: true });
    fs.writeFileSync(path.join(launcher, 'package.json'), JSON.stringify({ name: pkg.name, version: pkg.version, productName: pkg.productName, main: '../guard.cjs' }));
    fs.writeFileSync(preload, `const {contextBridge}=require('electron');contextBridge.exposeInMainWorld('ANALYTICS_DISABLED',true);try{localStorage.setItem('umami.disabled','1')}catch{}\n`);
    fs.writeFileSync(guard, `
const {app,session}=require('electron');
const http=require('node:http');
app.setPath('userData',${JSON.stringify(path.join(privateRoot, 'profile'))});
app.commandLine.appendSwitch('disable-gpu');
const listen=http.Server.prototype.listen;
http.Server.prototype.listen=function(...args){
 if(typeof args[0]==='object')args[0]={...args[0],host:'127.0.0.1'};
 else if(typeof args[1]==='string')args[1]='127.0.0.1';
 else args.splice(1,0,'127.0.0.1');
 const result=listen.apply(this,args);
 (globalThis.__referenceServers??=[]).push(this);
 return result;
};
app.once('ready',()=>{
 session.defaultSession.registerPreloadScript({type:'frame',filePath:${JSON.stringify(preload)}});
 session.defaultSession.webRequest.onBeforeRequest((details,cb)=>{
  const url=new URL(details.url); const allowed=url.protocol==='file:'||(['127.0.0.1','localhost'].includes(url.hostname)&&['14533','18433'].includes(url.port));
  cb({cancel:!allowed});
 });
});
require(${JSON.stringify(path.join(runtimeRoot, 'out/main/index.js'))});
`);
    report.buildFiles = {};
    for (const file of ['out/main/index.js','out/preload/index.js','out/renderer/index.html','out/remote/index.html']) report.buildFiles[file] = sha256(fs.readFileSync(path.join(runtimeRoot, file)));
    let app, browser, remoteContext, page;
    async function screenshot(target, id, note, mask = []) {
        await target.waitForLoadState('networkidle', { timeout: 5000 }).catch(() => {});
        await target.evaluate(() => document.fonts.ready);
        await target.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
        const maskedElementCount = (await Promise.all(mask.map((locator) => locator.count()))).reduce((a, b) => a + b, 0);
        if (mask.length) assert.ok(maskedElementCount > 0, 'Screenshot mask must match actual inputs');
        const file = id + '.png', domFile = id + '.txt';
        await target.screenshot({ path: path.join(outputRoot, file), animations: 'disabled', mask, maskColor: '#222222' });
        fs.writeFileSync(path.join(outputRoot, domFile), redact(await target.locator('body').ariaSnapshot(), secrets) + '\n');
        report.screenshots.push({ id, file, sha256: sha256(fs.readFileSync(path.join(outputRoot, file))), domFile, domSha256: sha256(fs.readFileSync(path.join(outputRoot, domFile))),
            viewport: await target.evaluate(() => ({ width: innerWidth, height: innerHeight })), maskedElementCount, note });
    }
    try {
        app = await pw._electron.launch({ executablePath: electronBinary, args: ['--no-sandbox', launcher],
            env: { ...process.env, DISABLE_AUTO_UPDATES: '1', XDG_CONFIG_HOME: path.join(privateRoot, 'config'), XDG_CACHE_HOME: path.join(privateRoot, 'cache') }, timeout: 30000 });
        page = await app.firstWindow(); page.setDefaultTimeout(20000);
        await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].setContentSize(1440, 1000));
        await page.getByRole('textbox', { name: 'Server Name', exact: true }).waitFor();
        report.environment.runtime = await app.evaluate(({ app }) => ({ name: app.getName(), version: app.getVersion(), electron: process.versions.electron, chromium: process.versions.chrome }));
        report.environment.analytics = await page.evaluate(() => ({ disabled: window.ANALYTICS_DISABLED, optOut: localStorage.getItem('umami.disabled') }));
        assert.equal('1.17.0', report.environment.runtime.version);
        assert.equal(true, report.environment.analytics.disabled);
        assert.equal('1', report.environment.analytics.optOut);
        await screenshot(page, 'desktop-onboarding', 'Actual Linux Electron renderer under Xvfb, before credential entry');
        await page.getByRole('textbox', { name: 'Server Name', exact: true }).fill('P00 Desktop');
        await page.getByRole('textbox', { name: 'URL', exact: true }).fill('http://127.0.0.1:14533');
        await page.getByRole('textbox', { name: 'Username', exact: true }).fill(credentials.admin);
        await page.getByRole('textbox', { name: 'Password', exact: true }).fill(credentials.password);
        await page.getByRole('button', { name: 'Add', exact: true }).click();
        await page.getByRole('link', { name: 'Tracks', exact: true }).waitFor();
        const release = page.getByRole('dialog', { name: /new version/ });
        await release.waitFor({ timeout: 7000 }); await release.getByRole('button').first().click(); await release.waitFor({ state: 'hidden' });
        await screenshot(page, 'desktop-home', 'Authenticated original desktop home; actual disposable Navidrome service');
        await page.getByRole('link', { name: 'Tracks', exact: true }).click();
        await page.getByRole('main').getByText('P00 Tone A', { exact: true }).waitFor();
        await screenshot(page, 'desktop-tracks', 'Original desktop track table with two owned tones');
        report.operations.push({ id: 'desktop-login-and-tracks', status: 'observed', trackNames: ['P00 Tone A','P00 Tone B'] });
        for (const title of ['P00 Tone A', 'P00 Tone B']) {
            await page.getByRole('main').getByText(title, { exact: true }).click({ button: 'right' });
            await page.getByRole('menuitem', { name: 'Play', exact: true }).last().hover();
            await page.getByRole('menuitem', { name: 'Last', exact: true }).click();
        }
        await page.evaluate(() => { location.hash = '/settings'; });
        await page.getByRole('tab', { name: 'Window', exact: true }).click();
        const row = (title) => page.locator('[class*="settings-option"][class*="row"]').filter({ has: page.getByText(title, { exact: true }) });
        const username = row('Remote control server username').locator('input');
        const password = row('Remote control server password').locator('input');
        secrets.add(await password.inputValue());
        const remotePassword = require('node:crypto').randomBytes(24).toString('base64url'); secrets.add(remotePassword);
        await username.fill('p00-remote'); await username.press('Tab');
        await password.fill(remotePassword); await password.press('Tab');
        const port = row('Remote control server port').locator('input');
        await port.fill('18433'); await port.press('Tab');
        await page.waitForFunction(() => JSON.parse(localStorage.getItem('store_settings')).state.remote.port === 18433);
        await row('Enable remote control server').getByRole('switch').focus();
        await row('Enable remote control server').getByRole('switch').press('Space');
        assert.equal(true, await row('Enable remote control server').getByRole('switch').isChecked());
        await screenshot(page, 'desktop-window-settings', 'Original desktop-only settings; all text inputs masked to protect remote credentials', [page.locator('input')]);
        report.environment.remoteListeners = await app.evaluate(() => (globalThis.__referenceServers || []).map((server) => server.address()).filter(Boolean));
        assert.ok(report.environment.remoteListeners.some((address) => address.address === '127.0.0.1' && address.port === 18433));
        browser = await pw.chromium.launch({ headless: true });
        remoteContext = await browser.newContext({ viewport: { width: 390, height: 844 }, httpCredentials: { username: 'p00-remote', password: remotePassword }, serviceWorkers: 'block' });
        await remoteContext.route('**/*', (route) => new URL(route.request().url()).origin === 'http://127.0.0.1:18433' ? route.continue() : route.abort());
        const remote = await remoteContext.newPage(); remote.setDefaultTimeout(25000);
        remote.on('websocket', (socket) => {
            for (const [event, direction] of [['framesent','sent'],['framereceived','received']]) socket.on(event, ({ payload }) => {
                try {
                    const value = JSON.parse(typeof payload === 'string' ? payload : payload.toString());
                    const metadata = value.event === 'operation-ack' ? value.data : value;
                    report.socketEvents.push({ direction, event: value.event, ...(metadata?.requestId && { requestId: metadata.requestId }), ...(value.event === 'operation-ack' && { error: metadata.error ?? null }) });
                } catch { report.socketEvents.push({ direction, event: '<non-JSON>' }); }
            });
        });
        await remote.goto('http://127.0.0.1:18433');
        await remote.getByRole('link', { name: 'Queue', exact: true }).waitFor();
        await screenshot(remote, 'remote-home', 'Actual authenticated mobile remote client connected to original desktop');
        await remote.getByRole('link', { name: 'Library', exact: true }).click();
        await remote.getByRole('listitem').filter({ hasText: 'P00 Tone A' }).waitFor();
        await remote.getByRole('listitem').filter({ hasText: 'P00 Tone B' }).waitFor();
        await screenshot(remote, 'remote-library', 'Original remote library view and desktop request forwarding');
        await remote.getByRole('link', { name: 'Queue', exact: true }).click();
        await remote.getByRole('button', { name: 'Clear Queue', exact: true }).waitFor();
        await remote.getByText('P00 Tone A', { exact: true }).waitFor();
        await remote.getByText('P00 Tone B', { exact: true }).waitFor();
        await screenshot(remote, 'remote-queue', 'Original desktop queue pushed to the authenticated remote');
        await remote.getByRole('button', { name: 'Clear Queue', exact: true }).click();
        await remote.getByRole('button', { name: 'Confirm', exact: true }).click();
        await remote.getByText('Queue is empty', { exact: true }).waitFor();
        await screenshot(remote, 'remote-cleared-queue', 'Clear Queue was confirmed in the remote; empty state observed');
        const request = report.socketEvents.find((event) => event.direction === 'sent' && event.event === 'clear-queue');
        const ack = report.socketEvents.find((event) => event.direction === 'received' && event.event === 'operation-ack' && event.requestId === request?.requestId);
        assert.ok(request?.requestId); assert.ok(ack); assert.equal(null, ack.error);
        const desktopQueue = await page.evaluate(async () => {
            const db = await new Promise((resolve) => { const request = indexedDB.open('keyval-store'); request.onsuccess = () => resolve(request.result); });
            try { const raw = await new Promise((resolve) => { const request = db.transaction('keyval').objectStore('keyval').get('player-store-queue'); request.onsuccess = () => resolve(request.result); }); return { present: typeof raw === 'string', count: raw ? JSON.parse(raw).default.length : null }; } finally { db.close(); }
        });
        assert.equal(true, desktopQueue.present);
        assert.equal(0, desktopQueue.count);
        report.operations.push({ id: 'remote-auth-queue-clear-ack', status: 'observed', requestId: request.requestId, acknowledgementError: ack.error, queueEmpty: true, desktopQueuePresent: desktopQueue.present, desktopQueueCount: desktopQueue.count });
        await row('Enable remote control server').getByRole('switch').focus();
        await row('Enable remote control server').getByRole('switch').press('Space');
    } catch (error) {
        if (page && !page.isClosed()) {
            for (const value of await page.locator('input').evaluateAll((nodes) => nodes.map((node) => node.value))) secrets.add(value);
            fs.writeFileSync(path.join(privateRoot, 'failure.json'), JSON.stringify({ error: redact(error.message, secrets), dom: redact(await page.locator('body').ariaSnapshot(), secrets) }, null, 2), { mode: 0o600 });
        }
        throw error;
    } finally {
        if (remoteContext) await remoteContext.close();
        if (browser) await browser.close();
        if (app) await app.close();
    }
    fs.writeFileSync(path.join(privateRoot, 'generated-secrets.json'), JSON.stringify([...secrets]), { mode: 0o600 });
    const clean = redact(report, secrets);
    fs.writeFileSync(path.join(outputRoot, 'desktop-run.json'), JSON.stringify(clean, null, 2) + '\n');
    return clean;
}
if (require.main === module) {
    const [playwright, electronBinary, runtimeRoot, sourceRoot, auditFile, credentialsFile, privateRoot, outputRoot] = process.argv.slice(2);
    if (!outputRoot) { console.error('Usage: node p00-desktop-capture.cjs PLAYWRIGHT_PACKAGE ELECTRON_BINARY RUNTIME_ROOT SOURCE_ROOT AUDIT_JSON CREDENTIALS PRIVATE_ROOT OUTPUT_ROOT'); process.exit(2); }
    capture({ playwright, electronBinary, runtimeRoot, sourceRoot, auditFile, credentialsFile, privateRoot, outputRoot }).then((report) => console.log(JSON.stringify({ screenshots: report.screenshots.length, operations: report.operations })))
        .catch((error) => { console.error('Desktop capture failed (' + error.name + '); inspect private session without publishing logs.'); process.exitCode = 1; });
}
module.exports = { capture };
