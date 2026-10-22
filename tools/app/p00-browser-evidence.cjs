'use strict';
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const COMMIT = '7d6ad318c79802e226996cfc39acba6b05bb22e8';
const sensitive = /^(password|pw|currentpw|newpw|credential|ndcredential|token|accesstoken|subsonictoken|subsonicsalt|authorization|xembytoken|xndauthorization|apikey|lastfmapikey|salt|secret)$/;
const normalized = (key) => key.toLowerCase().replace(/[^a-z0-9]/g, '');
const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');

function redact(value, secrets = []) {
    if (Array.isArray(value)) return value.map((item) => redact(item, secrets));
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) =>
        [key, sensitive.test(normalized(key)) && item ? '<redacted>' : redact(item, secrets)]));
    if (typeof value !== 'string') return value;
    // Persisted stores sometimes contain serialized JSON inside a string.
    if (/^[\[{]/.test(value)) {
        try { return JSON.stringify(redact(JSON.parse(value), secrets)); } catch { /* Ordinary text. */ }
    }
    if (/^https?:\/\//i.test(value)) {
        try {
            const url = new URL(value);
            // Authenticated image/stream URLs must not survive even under an innocuous key.
            let changed = Boolean(url.username || url.password);
            url.username = ''; url.password = '';
            for (const key of [...url.searchParams.keys()]) {
                if (sensitive.test(normalized(key)) || ['u', 'p', 's', 't', 'api_key'].includes(key.toLowerCase())) { url.searchParams.set(key, '<redacted>'); changed = true; }
            }
            if (changed) value = url.toString();
        } catch { /* Redact known values below as well. */ }
    }
    for (const secret of [...secrets].filter((s) => typeof s === 'string' && s.length >= 6).sort((a, b) => b.length - a.length)) value = value.split(secret).join('<redacted>');
    return value;
}

function learnSecrets(value, secrets) {
    if (Array.isArray(value)) { for (const item of value) learnSecrets(item, secrets); return; }
    if (!value || typeof value !== 'object') return;
    for (const [key, item] of Object.entries(value)) {
        if (sensitive.test(normalized(key)) && typeof item === 'string' && item.length >= 6) secrets.add(item);
        else learnSecrets(item, secrets);
    }
}

function requestRecord(request) {
    const url = new URL(request.url());
    // Body values are omitted by default: auth, tag edits and remote input can be sensitive.
    return { method: request.method(), origin: url.origin, path: url.pathname,
        queryKeys: [...new Set([...url.searchParams.keys()].filter((key) => !['u','p','s','t'].includes(key.toLowerCase()) && !sensitive.test(normalized(key))))].sort() };
}

// Resolve existing ancestors as well as symlinks when a fresh output path does not exist yet.
function canonicalPath(file) {
    let current = path.resolve(file);
    const suffix = [];
    while (true) {
        try { return path.join(fs.realpathSync(current), ...suffix.reverse()); }
        catch (error) {
            if (error.code !== 'ENOENT') throw error;
            const parent = path.dirname(current);
            if (parent === current) throw error;
            suffix.push(path.basename(current));
            current = parent;
        }
    }
}
function contains(root, file) {
    const relative = path.relative(root, file);
    return relative === '' || (relative !== '..' && !relative.startsWith('..' + path.sep) && !path.isAbsolute(relative));
}
function verifyOutputSeparation(privateRoot, outputRoot, credentialsFile) {
    const privatePath = canonicalPath(privateRoot), publicPath = canonicalPath(outputRoot);
    const privateName = path.resolve(privateRoot), publicName = path.resolve(outputRoot);
    // Keep lexical checks too: a public symlink to a private location is still publishable.
    if (contains(privatePath, publicPath) || contains(publicPath, privatePath) || contains(privateName, publicName) || contains(publicName, privateName)) throw new Error('Private and public capture roots must be disjoint');
    if (contains(publicPath, canonicalPath(credentialsFile)) || contains(publicName, path.resolve(credentialsFile))) throw new Error('Credentials must remain outside public capture output');
}

function verifySource(root, audit) {
    if (audit.commit !== COMMIT) throw new Error('Unexpected original source baseline');
    for (const [file, expected] of Object.entries(audit.sources)) {
        if (sha256(fs.readFileSync(path.join(root, file))) !== expected) throw new Error('Pinned source hash mismatch: ' + file);
    }
}
module.exports = { COMMIT, sha256, redact, learnSecrets, requestRecord, verifySource, verifyOutputSeparation };
