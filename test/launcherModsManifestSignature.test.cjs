const test = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('crypto');
const {
  buildManifestSignaturePayload,
  verifyLauncherModsManifestSignature,
} = require('../src/launcherModsManifestSignature.js');

test('buildManifestSignaturePayload — sortowanie en i znaki tab', () => {
  const manifest = {
    v: 1,
    generated: '2026-01-15T12:00:00.000Z',
    count: 2,
    mods: [
      { name: 'b.jar', size: 2, sha256: 'b'.repeat(64) },
      { name: 'a.jar', size: 1, sha256: 'a'.repeat(64) },
    ],
  };
  const payload = buildManifestSignaturePayload(manifest);
  const lines = payload.split('\n');
  assert.equal(lines[0], 'v1');
  assert.equal(lines[1], '2026-01-15T12:00:00.000Z');
  assert.equal(lines[2], '2');
  assert.ok(lines[3].localeCompare(lines[4], 'en') < 0, 'linie modów posortowane leksykograficznie');
});

test('verifyLauncherModsManifestSignature — zgodność z HMAC pierwszego sekretu', () => {
  const secret = 'unit-test-secret-min-16-chars';
  const manifest = {
    v: 1,
    generated: '2026-04-01T00:00:00.000Z',
    count: 1,
    mods: [{ name: 'Mod.jar', size: 100, sha256: 'c'.repeat(64) }],
    signature: '',
  };
  const payload = buildManifestSignaturePayload(manifest);
  manifest.signature = crypto.createHmac('sha256', secret).update(payload, 'utf8').digest('hex');
  assert.equal(verifyLauncherModsManifestSignature(manifest, secret), true);
  assert.equal(verifyLauncherModsManifestSignature(manifest, 'wrong-wrong-wrong-wrong-wrong-wrong'), false);
  manifest.signature = manifest.signature.slice(0, -1) + '0';
  assert.equal(verifyLauncherModsManifestSignature(manifest, secret), false);
});
