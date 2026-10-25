const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');

(async () => {
  const browser = await chromium.launch({headless:true, executablePath:process.env.CHROMIUM_PATH || undefined});
  try {
    const page = await browser.newPage({viewport:{width:1480, height:920}});
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.route('http://k8.test/**', route => {
      const name = new URL(route.request().url()).pathname.slice(1) || 'index.html';
      return route.fulfill({contentType:name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'text/html',
        body:fs.readFileSync(path.join(__dirname, '../ui', name), 'utf8')});
    });
    await page.addInitScript(() => {
      window.calls = [];
      window.deviceProfile = 0;
      window.failUpload = false;
      window.uploading = false;
      window.__TAURI__ = {core:{invoke:async (command, args) => {
        window.calls.push({command, args, duringUpload:window.uploading});
        if (command === 'frontend_report') return null;
        if (args.path === '/api/device') return {connected:true, keys:{A:4, Esc:41, LeftCtrl:224, LeftShift:225}, targets:[]};
        if (args.path === '/api/status') return {ok:true, status:{firmware:257, battery:{percent:50,state:'discharging'},
          lighting:{effect:'solid', color:'#ff6432', brightness:4, speed:2, slot:1, rainbow:false, direction:'right'},
          report_rate:1000, profile:window.deviceProfile, options:{win_lock:false},
          sleep:{bluetooth:60,wireless:60,bluetooth_deep:600,wireless_deep:600}, debounce:1}};
        if (args.path === '/api/progress') return {active:window.uploading, value:window.uploading ? .5 : 1, label:window.uploading ? 'Uploading' : 'Complete'};
        if (args.path === '/api/display' || args.path === '/api/display-frame') {
          window.uploading = true;
          await new Promise(resolve => setTimeout(resolve, 600));
          window.uploading = false;
          if (window.failUpload) throw 'Keyboard disconnected';
        }
        return {ok:true};
      }}};
    });
    await page.goto('http://k8.test/');
    await page.waitForFunction(() => state.status !== null && !state.refreshing);

    // Poll fresh device state without overwriting edits to the current profile.
    await page.selectOption('#effect', 'wave');
    await page.evaluate(async () => {
      const before = calls.filter(call => call.args?.path === '/api/status').length;
      await refreshDevice(true);
      if (calls.filter(call => call.args?.path === '/api/status').length !== before + 1) throw new Error('Status was not refreshed');
    });
    assert.equal(await page.locator('#effect').inputValue(), 'wave');
    await page.evaluate(async () => { window.deviceProfile = 1; await refreshDevice(true); });
    assert.equal(await page.evaluate(() => state.profile), 1);
    assert.equal(await page.locator('.profile-switch .active').getAttribute('data-profile'), '1');

    // Native audio receives the same selected effect and settings as the browser path.
    await page.selectOption('#effect', 'music_2');
    await page.locator('#brightness').fill('2');
    await page.selectOption('#direction', 'down');
    await page.click('#color-palette [data-color="#0A84FF"]');
    await page.click('#live-audio');
    await page.waitForFunction(() => !state.liveStarting && state.live.native);
    const native = await page.evaluate(() => calls.filter(call => call.args?.path === '/api/live-native-start').at(-1).args.bodyValue);
    assert.equal(native.lighting.effect, 'music_2');
    assert.equal(native.lighting.brightness, 2);
    assert.equal(native.lighting.direction, 'down');
    assert.equal(native.lighting.color, '#0a84ff');
    await page.click('#live-stop');

    await page.click('[data-page="macros"]');
    await page.click('#macro-record');
    await page.keyboard.down('Control');
    await page.keyboard.press('a');
    await page.keyboard.up('Control');
    await page.keyboard.press('Escape');
    const recorded = await page.evaluate(() => state.macroEvents.map(event => [event.key,event.action]));
    assert.deepEqual(recorded, [['LeftCtrl','down'],['A','down'],['A','up'],['LeftCtrl','up']]);
    await page.fill('#macro-index', '256');
    await page.click('#apply-macro');
    assert.equal(await page.evaluate(() => calls.some(call => call.args?.path === '/api/macro')), false);
    await page.fill('#macro-index', '0');
    await page.click('#apply-macro');
    assert.equal(await page.evaluate(() => calls.filter(call => call.args?.path === '/api/macro').at(-1).args.bodyValue.events.length), 4);
    await page.click('#macro-record');
    await page.keyboard.down('Shift');
    await page.keyboard.press('Escape');
    await page.keyboard.up('Shift');
    assert.deepEqual(await page.evaluate(() => state.macroEvents.map(event => [event.key,event.action])), [['LeftShift','down'],['LeftShift','up']]);

    // Invalid imports are atomic; damaged existing storage is recoverable on boot.
    await page.click('[data-page="settings"]');
    const preset = {name:'Saved',effect:'solid',color:'#123456',brightness:3,speed:2,rainbow:false,direction:'right',slot:1};
    await page.evaluate(preset => importPresets({version:1,presets:[preset]}), preset);
    await page.locator('#import-config').setInputFiles({name:'bad.json',mimeType:'application/json',buffer:Buffer.from('{"version":1,"presets":[null]}')});
    assert.deepEqual(await page.evaluate(() => JSON.parse(localStorage.getItem('k8-presets')).map(preset => preset.name)), ['Saved']);
    await page.evaluate(preset => localStorage.setItem('k8-presets', JSON.stringify([null,preset])), preset);
    assert.deepEqual(await page.evaluate(() => {
      const original = Storage.prototype.setItem;
      try {
        Storage.prototype.setItem = () => { throw new DOMException('Storage full', 'QuotaExceededError'); };
        return readPresets().map(preset => preset.name);
      } finally { Storage.prototype.setItem = original; }
    }), ['Saved']);
    await page.reload();
    await page.waitForFunction(() => state.status !== null);
    assert.deepEqual(await page.evaluate(() => readPresets().map(preset => preset.name)), ['Saved']);
    assert.ok(await page.evaluate(() => localStorage.getItem('k8-presets-recovery').includes('null')));

    await page.click('[data-page="display"]');
    assert.equal(await page.locator('#display-playback').count(), 0);
    assert.equal(await page.locator('#display-layer').isVisible(), true);
    assert.equal(await page.locator('#display-delay').isVisible(), false);
    await page.locator('#display-file').setInputFiles({name:'photo.png',mimeType:'image/png',buffer:Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jL1sAAAAASUVORK5CYII=','base64')});
    await page.click('#upload-display');
    await page.waitForFunction(() => window.uploading);
    assert.equal(await page.locator('#display-layer').isDisabled(), true);
    await page.waitForFunction(() => !window.k8Display.isBusy());
    let upload = await page.evaluate(() => calls.filter(call => call.args?.path === '/api/display').at(-1));
    assert.equal(upload.args.query.single, undefined);
    assert.equal(upload.args.query.layer, '0');
    assert.ok(await page.evaluate(() => calls.some(call => call.args?.path === '/api/progress' && call.duringUpload)));

    // Ordinary per-slot images and animated GIF uploads remain available.
    await page.selectOption('#display-layer', '4');
    await page.click('#upload-display');
    await page.waitForFunction(() => !window.k8Display.isBusy());
    upload = await page.evaluate(() => calls.filter(call => call.args?.path === '/api/display').at(-1));
    assert.equal(upload.args.query.single, undefined);
    assert.equal(upload.args.query.layer, '4');
    assert.equal(upload.args.query.delay, undefined);

    const gif = Buffer.from('47494638396101000100800000ff00000000ff21ff0b4e45545343415045322e30030100000021f904000a0000002c000000000100010000020244010021f904000a0000002c00000000010001000002024c01003b', 'hex');
    await page.locator('#display-file').setInputFiles({name:'motion.gif', mimeType:'image/gif', buffer:gif});
    assert.equal(await page.locator('#display-layer').isVisible(), false);
    assert.equal(await page.locator('#display-delay').isVisible(), true);
    await page.fill('#display-delay', '75');
    await page.click('#upload-display');
    await page.waitForFunction(() => !window.k8Display.isBusy());
    upload = await page.evaluate(() => calls.filter(call => call.args?.path === '/api/display').at(-1));
    assert.equal(upload.args.query.single, undefined);
    assert.equal(upload.args.query.delay, '75');
    assert.deepEqual(Buffer.from(upload.args.bytes), gif);
    await page.evaluate(() => { window.failUpload = true; });
    await page.click('#upload-display');
    await page.waitForFunction(() => !window.k8Display.isBusy());
    assert.equal(await page.locator('#upload-display').isDisabled(), false);
    assert.match(await page.locator('#toast-stack').innerText(), /Upload failed/);
    assert.deepEqual(errors, []);
    console.log('PASS: modifiers, macro boundaries, preset recovery, native audio settings, fresh profiles, unsaved edits, individual image slots, animated GIF uploads, progress and upload recovery.');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
