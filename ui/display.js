/* One-shot display tools. No timers capture or write LCD frames in the background. */
(() => {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 128;
  const context = canvas.getContext('2d', {willReadFrequently:true});
  let frame = null;
  let captured = null;
  let generation = 0;
  let busy = false;
  const source = () => $('#display-source').value;

  function updatePreview(label) {
    const preview = $('#display-preview');
    if (preview.src.startsWith('blob:')) URL.revokeObjectURL(preview.src);
    preview.src = canvas.toDataURL('image/png');
    preview.classList.add('visible');
    $('#display-placeholder').classList.add('hidden');
    $('#file-meta').textContent = label;
    $('#frame-meta').textContent = '128 × 128';
    frame = new Uint8Array(context.getImageData(0, 0, 128, 128).data);
    $('#upload-display').disabled = busy;
  }

  function clearPreview() {
    frame = null;
    const preview = $('#display-preview');
    if (preview.src.startsWith('blob:')) URL.revokeObjectURL(preview.src);
    preview.removeAttribute('src');
    preview.classList.remove('visible');
    $('#display-placeholder').classList.remove('hidden');
    $('#file-meta').textContent = 'No preview';
    $('#frame-meta').textContent = '—';
    $('#upload-display').disabled = true;
  }

  function renderCapture() {
    if (!captured) return;
    const scale = ($('#display-fit').value === 'cover' ? Math.max : Math.min)(128 / captured.width, 128 / captured.height);
    const w = captured.width * scale, h = captured.height * scale;
    context.fillStyle = '#000'; context.fillRect(0, 0, 128, 128);
    context.drawImage(captured, (128 - w) / 2, (128 - h) / 2, w, h);
    updatePreview('Screen snapshot');
  }

  async function capture() {
    const ticket = ++generation;
    let stream;
    try {
      if (navigator.mediaDevices?.getDisplayMedia && !window.__TAURI__) {
        stream = await navigator.mediaDevices.getDisplayMedia({video:{frameRate:1}, audio:false});
        const video = document.createElement('video');
        video.muted = true; video.srcObject = stream;
        await video.play();
        if (ticket !== generation || source() !== 'screen') return;
        const image = document.createElement('canvas');
        // Retain enough resolution for a centered crop without holding a full desktop image.
        const scale = Math.min(1, 512 / Math.max(video.videoWidth, video.videoHeight));
        image.width = Math.max(1, Math.round(video.videoWidth * scale));
        image.height = Math.max(1, Math.round(video.videoHeight * scale));
        image.getContext('2d').drawImage(video, 0, 0, image.width, image.height);
        captured = image;
      } else {
        const result = await post('/api/display-capture');
        if (ticket !== generation || source() !== 'screen') return;
        const image = new Image();
        await new Promise((resolve, reject) => {
          image.onload = resolve;
          image.onerror = () => reject(new Error('Invalid screen capture'));
          image.src = `data:image/png;base64,${result.png}`;
        });
        if (ticket !== generation || source() !== 'screen') return;
        captured = image;
      }
      renderCapture();
    } finally {
      if (stream) stream.getTracks().forEach(track => track.stop());
    }
  }

  function drawText(text, y, size, color) {
    context.font = `600 ${size}px system-ui, sans-serif`;
    context.fillStyle = color;
    context.fillText(text, 10, y, 108);
  }

  async function renderCard() {
    const ticket = ++generation;
    const kind = $('#display-card-type').value;
    $('#display-message-wrap').classList.toggle('hidden', kind !== 'note');
    frame = null; $('#upload-display').disabled = true;
    let lines = [];
    if (kind === 'system') {
      const data = await post('/api/system-card');
      const minutes = Math.floor(data.uptime_seconds / 60);
      lines = [`RAM  ${data.memory_percent}%`, `UP  ${Math.floor(minutes / 60)}h ${minutes % 60}m`];
    } else if (kind === 'datetime') {
      const now = new Date();
      lines = [now.toLocaleTimeString('en-GB', {hour:'2-digit', minute:'2-digit'}),
        now.toLocaleDateString('en-GB', {day:'2-digit', month:'short', year:'numeric'})];
    } else {
      // Wrap to the LCD width, preserving user-entered line breaks.
      context.font = '600 12px system-ui, sans-serif';
      for (const paragraph of $('#display-card-message').value.split('\n')) {
        let line = '';
        for (const char of paragraph) {
          if (context.measureText(line + char).width > 108) { lines.push(line); line = ''; }
          line += char;
        }
        lines.push(line);
      }
    }
    if (ticket !== generation || source() !== 'card') return;
    const accent = $('#display-card-color').value;
    context.fillStyle = '#101619'; context.fillRect(0, 0, 128, 128);
    context.fillStyle = accent; context.fillRect(10, 12, 28, 3);
    drawText($('#display-card-title').value, 36, 12, accent);
    lines.slice(0, 4).forEach((line, index) => drawText(line, 60 + index * 18, kind === 'datetime' && index === 0 ? 22 : 12, '#f2f5f4'));
    updatePreview('Info card');
  }

  function selectSource() {
    generation++;
    clearPreview();
    const value = source();
    ['image', 'screen', 'card', 'clock'].forEach(name => $(`#display-${name}-fields`).classList.toggle('hidden', name !== value));
    $('#display-delay-wrap').classList.toggle('hidden', value !== 'image');
    $('#display-slot-wrap').parentElement.classList.toggle('hidden', value === 'clock');
    $('#upload-display').classList.toggle('hidden', value === 'clock');
    $('.upload-progress').classList.toggle('hidden', value === 'clock');
    $('#upload-progress-bar').style.width = '0%';
    $('#upload-progress-label').textContent = 'Ready';
    if (value === 'image' && state.displayFile) {
      const preview = $('#display-preview');
      preview.src = URL.createObjectURL(state.displayFile); preview.classList.add('visible');
      $('#display-placeholder').classList.add('hidden');
      $('#file-meta').textContent = state.displayFile.name;
      $('#upload-display').disabled = false;
    } else if (value === 'card') renderCard().catch(error => toast('Could not create card', error.message, true));
    else if (value === 'screen') renderCapture();
    else if (value === 'clock') $('#file-meta').textContent = 'Built-in clock';
  }

  async function upload() {
    if (source() === 'clock' || busy) return;
    const isImage = source() === 'image';
    if (isImage ? !state.displayFile : !frame) return toast('Create a preview first', '', true);
    const data = isImage ? state.displayFile : frame.slice();
    const layer = $('#display-layer').value;
    busy = true;
    $('#display-source').disabled = true;
    stopLive();
    const timer = setInterval(pollProgress, 250);
    try {
      const query = new URLSearchParams({layer});
      const headers = {'Content-Type':'application/octet-stream'};
      if (isImage) {
        headers['X-Filename'] = encodeURIComponent(data.name);
        if ($('#display-delay').value) query.set('delay', $('#display-delay').value);
      }
      await withBusy($('#upload-display'), () => api(`/api/${isImage ? 'display' : 'display-frame'}?${query}`, {
        method:'POST', headers, body:data
      }));
      toast('Display updated', `Slot ${+layer + 1}`);
    } catch (error) { toast('Upload failed', error.message, true); }
    finally {
      clearInterval(timer); await pollProgress();
      busy = false; $('#display-source').disabled = false;
      $('#upload-display').disabled = source() === 'image' ? !state.displayFile : !frame;
    }
  }

  window.k8Display = {source, selectImage() {
    $('#display-source').value = 'image';
    selectSource();
    $('#upload-display').disabled = false;
  }};
  $('#display-source').addEventListener('change', selectSource);
  $('#display-fit').addEventListener('change', renderCapture);
  $('#display-capture').addEventListener('click', async event => {
    try { await withBusy(event.currentTarget, capture); }
    catch (error) { if (error.name !== 'NotAllowedError') toast('Could not capture screen', error.message, true); }
  });
  ['display-card-type', 'display-card-title', 'display-card-message', 'display-card-color'].forEach(id => {
    $(`#${id}`).addEventListener('change', () => renderCard().catch(error => toast('Could not create card', error.message, true)));
  });
  $('#display-card-refresh').addEventListener('click', () => renderCard().catch(error => toast('Could not create card', error.message, true)));
  $('#upload-display').addEventListener('click', upload);
  $('#display-sync-clock').addEventListener('click', async event => {
    try { await withBusy(event.currentTarget, () => post('/api/clock')); toast('Clock synced'); }
    catch (error) { toast('Could not sync clock', error.message, true); }
  });
  selectSource();
})();
