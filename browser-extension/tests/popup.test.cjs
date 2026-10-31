const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const crypto = require('node:crypto');
const path = require('node:path');
const root = path.join(__dirname, '..');
const manifest = JSON.parse(fs.readFileSync(path.join(root, 'manifest.json')));
const code = fs.readFileSync(path.join(root, 'popup.js'), 'utf8');
function scene(cookies, networkError = false) {
  const elements = Object.fromEntries(['connect', 'status', 'youtube'].map(id => [id, {addEventListener(event, handler) { this.click = handler; }}]));
  const calls = [];
  let reads = 0;
  vm.runInNewContext(code, {
    document: {getElementById: id => elements[id]}, AbortSignal, TypeError,
    chrome: {runtime: {id: 'mhcegfjhlladfhgpdoejkliodmkeooao'}, cookies: {getAll: async query => { reads++; assert.equal(query.url, 'https://music.youtube.com/'); return cookies; }}, tabs: {create() {}}},
    fetch: async (url, options) => { calls.push({url, options}); if(networkError) throw new TypeError('unavailable'); return {ok: true, json: async () => url.endsWith('challenge') ? {app:'Tubefast Stats',nonce:'a'.repeat(64)} : {ok:true}}; }
  });
  return {elements, calls, get reads() { return reads; }};
}
test('explicit connection sends only YouTube cookies over the local challenge', async () => {
  const state = scene([{name:'SAPISID',value:'synthetic',domain:'.youtube.com'}, {name:'SID',value:'foreign',domain:'.google.com'}]);
  assert.equal(state.reads, 0);
  await state.elements.connect.click();
  assert.equal(state.reads, 1);
  assert.equal(state.calls.length, 2);
  const request = state.calls[1];
  assert.equal(request.url, 'http://127.0.0.1:17889/v1/session');
  assert.equal(request.options.headers.Authorization, 'Bearer '+ 'a'.repeat(64));
  assert.equal(JSON.parse(request.options.body).cookies.length, 1);
  assert.match(state.elements.status.textContent, /Cuenta conectada/);
  assert.doesNotMatch(state.elements.status.textContent, /synthetic/);
});
test('missing browser login does not submit a session', async () => {
  const state = scene([]);
  await state.elements.connect.click();
  assert.equal(state.calls.length, 1);
  assert.match(state.elements.status.textContent, /Inicia sesión/);
  assert.equal(state.elements.connect.disabled, false);
});
test('a closed app never causes browser cookies to be read', async () => {
  const state = scene([], true);
  await state.elements.connect.click();
  assert.equal(state.reads, 0);
  assert.match(state.elements.status.textContent, /Abre Tubefast/);
});
test('the extension public key matches the native allowlist', () => {
  const hash = crypto.createHash('sha256').update(Buffer.from(manifest.key,'base64')).digest().subarray(0,16);
  const id = [...hash].map(b => String.fromCharCode(97+(b>>4),97+(b&15))).join('');
  const rust = fs.readFileSync(path.join(root,'..','src','extension_login.rs'),'utf8');
  assert.match(rust, new RegExp('EXTENSION_ID: &str = "'+id+'"'));
  assert.deepEqual(manifest.permissions, ['cookies']);
});
