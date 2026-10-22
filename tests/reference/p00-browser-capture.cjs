#!/usr/bin/env node
'use strict';
// Optional characterization of the original app, not application build tooling.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { COMMIT, sha256, redact, learnSecrets, requestRecord, verifySource, verifyOutputSeparation } = require('../../tools/app/p00-browser-evidence.cjs');

async function capture({ playwright, sourceRoot, auditFile, credentialsFile, privateRoot, outputRoot, fixtureIds }) {
    verifyOutputSeparation(privateRoot, outputRoot, credentialsFile);
    const auditBytes = fs.readFileSync(auditFile);
    const audit = JSON.parse(auditBytes);
    verifySource(sourceRoot, audit);
    const credentials = JSON.parse(fs.readFileSync(credentialsFile));
    const secrets = new Set([credentials.password, credentials.reader_password]);
    fs.mkdirSync(privateRoot, { recursive: true, mode: 0o700 });
    fs.mkdirSync(outputRoot, { recursive: true });
    const report = { schema: 1, tasks: ['P00.03', 'P00.04', 'P00.06'], commit: COMMIT,
        evidenceKind: 'original-production-web-browser-run', sourceAuditSha256: sha256(auditBytes),
        date: new Date().toISOString(), screenshots: [], operations: [], requests: [], servedAssets: {}, serverDiscovery: {}, externalRequestsBlocked: [], pageErrors: [],
        environment: { playwright: require(path.join(playwright, 'package.json')).version, node: process.version, viewport: { width: 1440, height: 1000 },
            headless: true, browserServiceWorkers: 'blocked', audioOutput: 'browser default; physical output/loopback not observed' },
        limitations: ['Screenshots and DOM/input/network observations concern the original production web build, not Electron or Rust.',
            'Disposable two-tone servers; no real library/profile. Only listed operations were executed.',
            'Analytics disabled before first HTML script and nonloopback browser requests blocked.',
            'No physical audio, hardware frame/input latency or desktop minimized/tray performance claim.',
            'Private persistent profiles and original unredacted downloads remain outside tracked evidence.'] };
    const pw = require(playwright);
    const fixtures = [
        { id: 'navidrome-current', type: 'Navidrome', port: 14533 },
        { id: 'navidrome-old', type: 'Navidrome', port: 14534 },
        { id: 'gonic-current', type: 'OpenSubsonic', port: 14747, username: 'admin' },
        { id: 'gonic-old', type: 'OpenSubsonic', port: 14748, username: 'admin' },
        { id: 'jellyfin-current', type: 'Jellyfin', port: 18096 },
        { id: 'jellyfin-old', type: 'Jellyfin', port: 18097 },
    ].filter((f) => !fixtureIds || fixtureIds.includes(f.id));
    for (const fixture of fixtures) {
        const context = await pw.chromium.launchPersistentContext(path.join(privateRoot, fixture.id), {
            headless: true, viewport: report.environment.viewport, serviceWorkers: 'block', acceptDownloads: true,
        });
        report.environment.chromium = context.browser().version();
        const page = context.pages()[0] || await context.newPage();
        page.setDefaultTimeout(15000);
        let operation = 'startup';
        const pending = [];
        async function screenshot(name, note) {
            await page.waitForLoadState('networkidle', { timeout: 5000 }).catch(() => {});
            await page.evaluate(() => document.fonts.ready);
            await page.mouse.move(1435, 5);
            const id = fixture.id + '-' + name;
            const file = id + '.png';
            await page.screenshot({ path: path.join(outputRoot, file), animations: 'disabled' });
            const domFile = id + '.txt';
            fs.writeFileSync(path.join(outputRoot, domFile), redact(await page.locator('body').ariaSnapshot(), secrets) + '\n');
            report.screenshots.push({ id, fixture: fixture.id, file, sha256: sha256(fs.readFileSync(path.join(outputRoot, file))), domFile,
                domSha256: sha256(fs.readFileSync(path.join(outputRoot, domFile))), route: new URL(page.url()).hash || '#/', note });
        }
        async function step(id, action) {
            operation = id;
            try {
                const observed = await action();
                report.operations.push({ fixture: fixture.id, id, status: 'observed', observed });
                return true;
            } catch (error) {
                report.operations.push({ fixture: fixture.id, id, status: 'failed', errorClass: error.name,
                    diagnostic: redact(error.message.split('\n')[0], secrets) });
                await screenshot(id + '-failure', 'Failed harness operation; not a passed workflow').catch(() => {});
                return false;
            }
        }
        async function settle() {
            await page.waitForLoadState('networkidle', { timeout: 10000 }).catch(() => {});
        }
        async function dismissReleaseNotes() {
            if (await page.evaluate(() => localStorage.getItem('version') !== '"1.17.0"')) {
                const dialog = page.getByRole('dialog', { name: /new version/ });
                await dialog.waitFor({ timeout: 6000 });
                await dialog.getByRole('button').first().click();
                await dialog.waitFor({ state: 'hidden' });
            }
        }
        async function settingsAdvanced() {
            await page.evaluate(() => { location.hash = '/settings'; });
            await page.getByRole('tab', { name: 'Advanced', exact: true }).click();
            await page.getByRole('button', { name: 'Export settings', exact: true }).waitFor();
        }
        async function exportSettings(name) {
            const downloadPromise = page.waitForEvent('download');
            await page.getByRole('button', { name: 'Export settings', exact: true }).click();
            const download = await downloadPromise;
            const privateFile = path.join(privateRoot, fixture.id + '-' + name + '.json');
            await download.saveAs(privateFile);
            fs.chmodSync(privateFile, 0o600);
            const bytes = fs.readFileSync(privateFile);
            const value = JSON.parse(bytes);
            learnSecrets(value, secrets);
            const file = fixture.id + '-' + name + '.json';
            fs.writeFileSync(path.join(outputRoot, file), JSON.stringify(redact(value, secrets), null, 2) + '\n');
            return { value, file, sha256: sha256(fs.readFileSync(path.join(outputRoot, file))) };
        }
        async function addLast(title) {
            await page.getByRole('main').getByText(title, { exact: true }).first().click({ button: 'right' });
            await page.getByRole('menuitem', { name: 'Play', exact: true }).last().hover();
            await page.getByRole('menuitem', { name: 'Last', exact: true }).click();
        }
        async function queueState() {
            return page.evaluate(async () => {
                const db = await new Promise((resolve, reject) => { const request = indexedDB.open('keyval-store'); request.onsuccess = () => resolve(request.result); request.onerror = () => reject(request.error); });
                try {
                    const store = db.transaction('keyval').objectStore('keyval');
                    const raw = await new Promise((resolve, reject) => { const request = store.get('player-store-queue'); request.onsuccess = () => resolve(request.result); request.onerror = () => reject(request.error); });
                    if (!raw) return null;
                    const q = JSON.parse(raw);
                    return { default: q.default, shuffled: q.shuffled, priority: q.priority,
                        songs: Object.fromEntries(Object.entries(q.songs).map(([id, song]) => [id, { id: song.id, name: song.name, serverId: song._serverId, durationMs: song.duration }])) };
                } finally { db.close(); }
            });
        }
        async function clickTooltip(label) {
            const buttons = page.locator('.media-player').getByRole('button');
            for (const button of await buttons.all()) {
                if (!await button.isVisible()) continue;
                await button.hover();
                const tooltip = page.getByRole('tooltip', { name: label, exact: true });
                try { await tooltip.waitFor({ timeout: 180 }); await button.click(); return; } catch { /* Next visible control. */ }
            }
            throw new Error('Could not find original player tooltip: ' + label);
        }
        try {
            await context.addInitScript(() => { localStorage.setItem('umami.disabled', '1'); window.ANALYTICS_DISABLED = true; });
            await context.route('**/*', (route) => {
                const url = new URL(route.request().url());
                if (['127.0.0.1', 'localhost'].includes(url.hostname) && ['18180','18181','14533','14534','14747','14748','18096','18097'].includes(url.port)) return route.continue();
                report.externalRequestsBlocked.push({ fixture: fixture.id, origin: url.origin, path: url.pathname });
                return route.abort();
            });
            page.on('pageerror', (error) => report.pageErrors.push({ fixture: fixture.id, operation, name: error.name, message: redact(error.message, secrets) }));
            page.on('response', (response) => {
                const url = new URL(response.url());
                if (url.port === '18180' && response.status() === 200 && /\.(js|css|woff2)$|^\/$/.test(url.pathname)) {
                    pending.push(response.body().then((bytes) => {
                        const receipt = { sha256: sha256(bytes), bytes: bytes.length };
                        if (report.servedAssets[url.pathname]) assert.deepEqual(report.servedAssets[url.pathname], receipt);
                        report.servedAssets[url.pathname] = receipt;
                    }));
                }
                if (url.port !== String(fixture.port)) return;
                const snapshot = { fixture: fixture.id, operation, ...requestRecord(response.request()), status: response.status() };
                if (/scrobble|reportPlayback|sessions\/playing/i.test(url.pathname)) {
                    const query = Object.fromEntries(url.searchParams);
                    let body;
                    try { body = response.request().postDataJSON(); } catch { body = null; }
                    const allow = new Set(['id', 'submission', 'position', 'positionMs', 'state', 'event', 'ItemId', 'PositionTicks', 'EventName', 'IsPaused']);
                    snapshot.playback = Object.fromEntries(Object.entries({ ...query, ...(body || {}) }).filter(([key]) => allow.has(key)));
                }
                report.requests.push(snapshot);
                pending.push(response.json().then((body) => {
                    learnSecrets(body, secrets);
                    if (body?.['subsonic-response']?.status === 'ok' && /ping/.test(url.pathname)) {
                        const envelope = body['subsonic-response'];
                        report.serverDiscovery[fixture.id] = { protocolVersion: envelope.version, serverVersion: envelope.serverVersion ?? null, openSubsonic: envelope.openSubsonic ?? false };
                    } else if (body?.Version && /system\/info/i.test(url.pathname)) report.serverDiscovery[fixture.id] = { serverVersion: body.Version };
                }).catch(() => {}));
            });
            const loggedIn = await step('login', async () => {
                await page.goto('http://127.0.0.1:18180');
                await page.getByRole('textbox', { name: 'Server Name', exact: true }).waitFor();
                await page.getByText(fixture.type, { exact: true }).click();
                await screenshot('onboarding', 'Fresh isolated original profile; credentials have not been entered');
                await page.getByRole('textbox', { name: 'Server Name', exact: true }).fill('P00 ' + fixture.type);
                await page.getByRole('textbox', { name: 'URL', exact: true }).fill('http://127.0.0.1:' + fixture.port);
                await page.getByRole('textbox', { name: 'Username', exact: true }).fill(fixture.username || credentials.admin);
                await page.getByRole('textbox', { name: 'Password', exact: true }).fill(credentials.password);
                await page.getByRole('button', { name: 'Add', exact: true }).click();
                await page.getByRole('link', { name: 'Tracks', exact: true }).waitFor({ timeout: 30000 });
                await settle(); await dismissReleaseNotes();
                await screenshot('home', 'Authenticated home, actual disposable service responses; release notes dismissed through UI');
                return { route: new URL(page.url()).hash, loginUi: true };
            });
            if (!loggedIn) continue;
            await step('tracks', async () => {
                await page.getByRole('link', { name: 'Tracks', exact: true }).click();
                await page.getByRole('main').getByText('P00 Tone A', { exact: true }).waitFor();
                await page.getByRole('main').getByText('P00 Tone B', { exact: true }).waitFor();
                await settle(); await screenshot('tracks', 'Two owned tones in the original virtualized track table');
                return { visibleTrackNames: ['P00 Tone A', 'P00 Tone B'] };
            });
            await step('albums', async () => {
                await page.getByRole('link', { name: 'Albums', exact: true }).click();
                await page.getByRole('main').getByText('Fixture Album', { exact: true }).first().waitFor();
                await settle(); await screenshot('albums', 'Original album browsing against the disposable fixture');
                await page.getByRole('main').getByText('Fixture Album', { exact: true }).first().click();
                await page.getByRole('main').getByText('P00 Tone A', { exact: true }).first().waitFor();
                await settle(); await screenshot('album-detail', 'Original album detail and track list');
                return { album: 'Fixture Album' };
            });
            if (fixture.id !== 'navidrome-current') continue;
            for (const [id, route, heading] of [['artists','/library/album-artists','Album Artists'], ['genres','/library/genres','Genres'], ['favorites','/favorites','Favorites'], ['playlists','/playlists','Playlists'], ['radio','/radio','Radio stations']]) {
                await step(id, async () => {
                    await page.evaluate((route) => { location.hash = route; }, route);
                    await page.getByRole('main').getByRole('heading', { name: heading, exact: true }).waitFor();
                    await settle(); await screenshot(id, 'Direct hash navigation in the original app; two-tone fixture data');
                    return { route };
                });
            }
            await step('keyboard-selection-and-duplicate-queue', async () => {
                await page.getByRole('link', { name: 'Tracks', exact: true }).click();
                const a = page.getByRole('main').getByText('P00 Tone A', { exact: true });
                await a.click();
                await page.getByRole('main').locator('[class*="row-selected"]').filter({ hasText: 'P00 Tone A' }).first().waitFor();
                await page.keyboard.press('ArrowDown');
                await page.getByRole('main').locator('[class*="row-selected"]').filter({ hasText: 'P00 Tone B' }).first().waitFor();
                const selectedText = await page.getByRole('main').locator('[class*="row-selected"]').allTextContents();
                assert.ok(selectedText.some((text) => text.includes('P00 Tone B')));
                await screenshot('keyboard-selection', 'Clicked first track then pressed ArrowDown; DOM selection observed separately');
                await addLast('P00 Tone A'); await addLast('P00 Tone A'); await addLast('P00 Tone B');
                await page.waitForFunction(async () => {
                    const db = await new Promise((resolve) => { const request = indexedDB.open('keyval-store'); request.onsuccess = () => resolve(request.result); });
                    try { const raw = await new Promise((resolve) => { const request = db.transaction('keyval').objectStore('keyval').get('player-store-queue'); request.onsuccess = () => resolve(request.result); }); return raw && JSON.parse(raw).default.length === 3; } finally { db.close(); }
                });
                const queue = await queueState();
                assert.equal(3, queue.default.length);
                assert.notEqual(queue.default[0], queue.default[1]);
                assert.equal(queue.songs[queue.default[0]].id, queue.songs[queue.default[1]].id);
                await clickTooltip('View queue');
                await screenshot('duplicate-queue', 'Two distinct persisted occurrences of the same track followed by a second track');
                fs.writeFileSync(path.join(outputRoot, 'browser-queue.json'), JSON.stringify(queue, null, 2) + '\n');
                return { selectedText, occurrences: queue.default.length, distinctOccurrenceIds: new Set(queue.default).size, repeatedSongId: true };
            });
            await step('drag-queue-reorder', async () => {
                const before = await queueState();
                const movedId = before.default[2];
                const panel = page.locator('#sidebar-play-queue-container');
                await panel.getByText('P00 Tone B', { exact: true }).dragTo(panel.getByText('P00 Tone A', { exact: true }).first(), { targetPosition: { x: 5, y: 2 } });
                await page.waitForFunction(async (id) => {
                    const db = await new Promise((resolve) => { const request = indexedDB.open('keyval-store'); request.onsuccess = () => resolve(request.result); });
                    try { const raw = await new Promise((resolve) => { const request = db.transaction('keyval').objectStore('keyval').get('player-store-queue'); request.onsuccess = () => resolve(request.result); }); return raw && JSON.parse(raw).default[0] === id; } finally { db.close(); }
                }, movedId);
                const after = await queueState();
                assert.deepEqual([...after.default].sort(), [...before.default].sort());
                await screenshot('queue-after-drag', 'Actual mouse drag moves the final queue occurrence before the first; persisted IDs preserved');
                fs.writeFileSync(path.join(outputRoot, 'browser-queue-after-drag.json'), JSON.stringify(after, null, 2) + '\n');
                return { beforeIds: before.default, afterIds: after.default, movedId };
            });
            await step('queue-reload', async () => {
                const before = await queueState();
                await page.reload(); await page.getByRole('link', { name: 'Tracks', exact: true }).waitFor();
                const after = await queueState();
                assert.deepEqual(after.default, before.default);
                return { beforeIds: before.default, afterIds: after.default, restored: true };
            });
            await step('browser-playback-and-space-pause', async () => {
                await page.keyboard.press('Escape');
                await page.getByRole('main').getByText('P00 Tone A', { exact: true }).first().dblclick();
                await page.getByRole('dialog', { name: 'Discard the current queue?', exact: true }).getByRole('button', { name: 'Confirm', exact: true }).click();
                await page.waitForFunction(() => [...document.querySelectorAll('audio')].some((audio) => !audio.paused && audio.currentTime > 0.25));
                const before = await page.locator('audio').evaluateAll((nodes) => nodes.map((node) => ({ currentTimeSec: node.currentTime, durationSec: Number.isFinite(node.duration) ? node.duration : null, paused: node.paused, readyState: node.readyState })));
                await page.getByRole('main').getByRole('heading', { name: 'Tracks', exact: true }).click();
                await page.keyboard.press('Space');
                await page.locator('.player-state-paused').waitFor();
                // Original WebPlayer fades for 300 ms before pausing the media element.
                // Bound this below the 3-second tone end so natural end cannot satisfy pause.
                await page.waitForFunction(() => {
                    const nodes = [...document.querySelectorAll('audio')];
                    return nodes.length > 0 && nodes.every((node) => node.paused) && nodes.some((node) => node.currentTime > 0 && !node.ended && node.currentTime < node.duration);
                }, null, { timeout: 1000 });
                await screenshot('paused-playback', 'Real Chromium media progress followed by Space hotkey pause; physical audio output unverified');
                const after = await page.locator('audio').evaluateAll((nodes) => nodes.map((node) => ({ currentTimeSec: node.currentTime, durationSec: Number.isFinite(node.duration) ? node.duration : null, ended: node.ended, paused: node.paused })));
                assert.ok(after.length > 0 && after.every((node) => node.paused));
                return { before, after, pausedButton: true };
            });
            await step('settings-export-import-reload', async () => {
                await settingsAdvanced(); await screenshot('advanced-settings', 'Original advanced settings, analytics disabled before startup');
                const exported = await exportSettings('settings-export');
                assert.equal(34, exported.value.version);
                await page.getByRole('button', { name: 'Import settings', exact: true }).click();
                const input = page.locator('input[type=file]');
                await input.setInputFiles({ name: 'invalid.json', mimeType: 'application/json', buffer: Buffer.from('{not json') });
                await page.getByText('The file passed is not valid JSON', { exact: false }).waitFor();
                await screenshot('invalid-settings-import', 'Actual import validation rejects malformed JSON');
                const changed = JSON.parse(JSON.stringify(exported.value)); changed.general.resume = false;
                await input.setInputFiles({ name: 'changed-settings.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(changed)) });
                await page.getByRole('dialog').getByRole('button', { name: 'Import settings', exact: true }).waitFor();
                await screenshot('settings-import-preview', 'Derived current-version export changes only general.resume; not a historical profile');
                await page.getByRole('dialog').getByRole('button', { name: 'Import settings', exact: true }).click();
                await page.getByText('Settings have been imported successfully!', { exact: false }).waitFor();
                await screenshot('settings-import-complete', 'Original UI reports import completion');
                await page.getByRole('dialog').getByRole('button').first().click();
                const applied = await exportSettings('settings-after-import');
                assert.equal(false, applied.value.general.resume);
                await page.reload(); await page.getByRole('tab', { name: 'Advanced', exact: true }).click();
                const reloaded = await exportSettings('settings-after-reload');
                const storageKeys = await page.evaluate(() => Object.keys(localStorage).filter((key) => key.startsWith('store_settings')).sort());
                assert.deepEqual(storageKeys, ['store_settings']);
                assert.equal(false, reloaded.value.general.resume);
                return { version: exported.value.version, downloadedFile: exported.file, downloadedSha256: exported.sha256,
                    derivedInput: { general: { resume: false } }, appliedResume: applied.value.general.resume, reloadedResume: reloaded.value.general.resume,
                    afterImportFile: applied.file, afterReloadFile: reloaded.file, storageKeys, currentVersionOnly: true };
            });
        } finally {
            const results = await Promise.allSettled(pending);
            await context.close();
            if (results.some((result) => result.status === 'rejected')) throw new Error('Served asset receipt collection failed');
        }
    }
    fs.writeFileSync(path.join(privateRoot, 'generated-secrets.json'), JSON.stringify([...secrets]), { mode: 0o600 });
    // Final pass uses all learned response and generated setting credentials, including earlier records.
    for (const item of report.screenshots) {
        const file = path.join(outputRoot, item.domFile);
        fs.writeFileSync(file, redact(fs.readFileSync(file, 'utf8'), secrets));
        item.domSha256 = sha256(fs.readFileSync(file));
    }
    const clean = redact(report, secrets);
    fs.writeFileSync(path.join(outputRoot, 'browser-run.json'), JSON.stringify(clean, null, 2) + '\n');
    return clean;
}
if (require.main === module) {
    const [playwright, sourceRoot, auditFile, credentialsFile, privateRoot, outputRoot, fixtureList] = process.argv.slice(2);
    if (!outputRoot) { console.error('Usage: node p00-browser-capture.cjs PLAYWRIGHT_PACKAGE SOURCE_ROOT AUDIT_JSON CREDENTIALS PRIVATE_ROOT OUTPUT_ROOT [FIXTURE_IDS]'); process.exit(2); }
    capture({ playwright, sourceRoot, auditFile, credentialsFile, privateRoot, outputRoot, fixtureIds: fixtureList?.split(',') }).then((report) => {
        if (report.operations.some((item) => item.status !== 'observed')) process.exitCode = 1;
        console.log(JSON.stringify({ screenshots: report.screenshots.length, operations: report.operations.map(({ fixture, id, status }) => ({ fixture, id, status })), requests: report.requests.length }));
    }).catch((error) => { console.error('Capture failed (' + error.name + '); inspect private session without publishing logs.'); process.exitCode = 1; });
}
module.exports = { capture };
