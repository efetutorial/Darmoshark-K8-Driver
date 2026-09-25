const {chromium}=require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const fs=require('node:fs');
const assert=require('node:assert/strict');
(async()=>{
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM_PATH || undefined});
const page=await browser.newPage({viewport:{width:1480,height:920}});
const errors=[];page.on('pageerror',e=>errors.push(e.message));
const requests=[];
await page.route('http://k8.test/**',route=>{
const path=new URL(route.request().url()).pathname;
if(path.startsWith('/api/')) {
 if(route.request().method()==='POST') {
 if(path === '/api/display-frame') requests.push({frameBytes:route.request().postDataBuffer().length});
 else requests.push(JSON.parse(route.request().postData()));
 }
 if(path === '/api/system-card') return route.fulfill({json:{ok:true,memory_percent:42,uptime_seconds:7500}});
return route.fulfill({json:path==='/api/device'?{connected:false,keys:{A:4,Esc:41},targets:[]}:{ok:true}});
}
const name=path==='/'?'index.html':path.slice(1);
return route.fulfill({contentType:name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'text/html',body:fs.readFileSync(require('node:path').join(__dirname,'../ui',name),'utf8')});
});
await page.goto('http://k8.test/');
await page.waitForFunction(()=>document.querySelectorAll('#effect option').length>10);
assert.equal(await page.locator('html').getAttribute('lang'),'en');
await page.selectOption('#effect','solid');
assert.equal(await page.locator('#speed').isVisible(),false);
await page.selectOption('#effect','wave');
await page.check('#rainbow',{force:true});
assert.equal(await page.locator('#color').isVisible(),true);
await page.click('#apply-lighting');
assert.equal(requests.at(-1).rainbow,true);
await page.click('#color-palette [data-color="#0A84FF"]');
assert.equal(await page.locator('#rainbow').isChecked(),false);
await page.click('#apply-lighting');
assert.equal(requests.at(-1).color,'#0a84ff');
await page.selectOption('#effect','neon');
for(const id of ['color','rainbow','direction'])assert.equal(await page.locator('#'+id).isVisible(),false,id);
assert.equal(await page.locator('#speed').isVisible(),true);
await page.click('#apply-lighting');
assert.equal(requests.at(-1).rainbow,false);
await page.selectOption('#effect','snake');
assert.deepEqual(await page.locator('#direction option').allTextContents(),['Zigzag','Return']);
await page.selectOption('#effect','music_2');
assert.deepEqual(await page.locator('#direction option').allTextContents(),['Upright','Separate','Intersect']);
assert.equal(await page.locator('#speed').isVisible(),false);
await page.selectOption('#effect','user_picture');
await page.selectOption('#rgb-slot','4');await page.click('#apply-lighting');assert.equal(requests.at(-1).slot,4);
for(const id of ['color','rainbow','speed','direction'])assert.equal(await page.locator('#'+id).isVisible(),false);
await page.selectOption('#effect','off');
assert.equal(await page.locator('#brightness').isVisible(),false);
await page.selectOption('#effect','wave');

for(const name of ['display','keys','macros','settings']){
 await page.click(`[data-page="${name}"]`);
 assert.equal(await page.locator('#'+name).isVisible(),true);
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 assert.equal(/[ışğüöçİŞĞÜÖÇ]/.test(await page.locator('body').innerText()),false);
}

await page.click('[data-page="display"]');
await page.selectOption('#display-source','card');
assert.equal(await page.locator('#display-preview').isVisible(),true);
await page.fill('#display-card-message','Test card');
if(process.env.K8_SCREENSHOT) await page.screenshot({path:process.env.K8_SCREENSHOT,fullPage:true});
await page.click('#display-card-refresh');
await page.click('#upload-display');
assert.equal(requests.at(-1).frameBytes,128*128*4);
await page.selectOption('#display-card-type','system');
await page.waitForFunction(()=>!document.querySelector('#upload-display').disabled);
await page.selectOption('#display-source','clock');
assert.equal(await page.locator('#upload-display').isVisible(),false);
await page.click('#display-sync-clock');
await page.selectOption('#display-source','screen');
assert.equal(await page.locator('#display-capture').isVisible(),true);
assert.equal(await page.locator('#upload-display').isDisabled(),true);
await page.evaluate(()=>{
 const canvas=document.createElement('canvas');canvas.width=canvas.height=128;
 const ctx=canvas.getContext('2d');ctx.fillStyle='red';ctx.fillRect(0,0,128,128);
 window.testCaptureStream=canvas.captureStream(1);
 Object.defineProperty(navigator,'mediaDevices',{value:{getDisplayMedia:async()=>window.testCaptureStream},configurable:true});
});
await page.click('#display-capture');
await page.waitForFunction(()=>!document.querySelector('#upload-display').disabled);
assert.equal(await page.evaluate(()=>window.testCaptureStream.getTracks().every(t=>t.readyState==='ended')),true);
await page.click('#upload-display');
assert.equal(requests.at(-1).frameBytes,128*128*4);
await page.selectOption('#display-source','image');
await page.locator('#display-file').setInputFiles({name:'test.png',mimeType:'image/png',buffer:Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jL1sAAAAASUVORK5CYII=','base64')});
assert.equal(await page.locator('#upload-display').isDisabled(),false);
assert.equal(await page.locator('#display-preview').isVisible(),true);
await page.setViewportSize({width:1050,height:700});await page.click('[data-page="lighting"]');
assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
assert.deepEqual(errors,[]);

// Exercise the packaged Tauri path as well as the browser-development fallback.
const nativePage=await browser.newPage({viewport:{width:1050,height:700}});
await nativePage.route('http://k8.test/**',route=>{
 const path=new URL(route.request().url()).pathname;
 const name=path==='/'?'index.html':path.slice(1);
 return route.fulfill({contentType:name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'text/html',body:fs.readFileSync(require('node:path').join(__dirname,'../ui',name),'utf8')});
});
await nativePage.addInitScript(()=>{
 window.nativeCalls=[];
 window.__TAURI__={core:{invoke:async(command,args)=>{
  window.nativeCalls.push({command,args});
  if(command==='frontend_report')return null;
  if(args.path==='/api/device')return {connected:false,keys:{A:4,Esc:41},targets:[]};
  if(args.path==='/api/progress')return {active:false,value:0,label:'Ready'};
  if(args.path==='/api/system-card')return {ok:true,memory_percent:42,uptime_seconds:7500};
  return {ok:true};
 }}};
});
await nativePage.goto('http://k8.test/');
await nativePage.waitForFunction(()=>window.nativeCalls.some(call=>call.args?.path==='/api/device'));
await nativePage.click('#live-screen');
await nativePage.waitForFunction(()=>window.nativeCalls.some(call=>call.args?.path==='/api/live-native-start'));
assert.equal(await nativePage.locator('#lighting-live-label').textContent(),'SCREEN SYNC');
await nativePage.click('#live-stop');
await nativePage.waitForFunction(()=>window.nativeCalls.some(call=>call.args?.path==='/api/live-native-stop'));
await nativePage.click('[data-page="display"]');
await nativePage.locator('#display-file').setInputFiles({name:'native.png',mimeType:'image/png',buffer:Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jL1sAAAAASUVORK5CYII=','base64')});
await nativePage.click('#upload-display');
await nativePage.waitForFunction(()=>window.nativeCalls.some(call=>call.args?.path==='/api/display'));
const nativeUpload=await nativePage.evaluate(()=>window.nativeCalls.find(call=>call.args?.path==='/api/display'));
assert.equal(nativeUpload.command,'api');
assert.equal(nativeUpload.args.query.filename,'native.png');
assert.equal(nativeUpload.args.bytes.length,68);
assert.equal(nativeUpload.args.bodyValue,null);
await nativePage.close();
console.log('PASS: navigation, English text, mode controls, normalized requests, display cards, snapshots, capture cleanup, desktop layouts; no browser errors.');
await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});
