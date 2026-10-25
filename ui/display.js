/* LCD photo and animated GIF upload. */
(() => {
  let busy = false;
  const imageIsGif = () => state.displayFile && (state.displayFile.type === 'image/gif' || /\.gif$/i.test(state.displayFile.name));

  function selectImage() {
    const gif = imageIsGif();
    $('#display-slot-wrap').classList.toggle('hidden', !!gif);
    $('#display-delay-wrap').classList.toggle('hidden', !gif);
    $('#display-content-note').textContent = gif
      ? 'GIF files keep all frames (up to 222). The keyboard uses one delay for the animation.'
      : 'Only the selected photo slot is replaced.';
    $('#upload-display').disabled = busy || !state.displayFile;
    $('#upload-progress-bar').style.width = '0%';
    $('#upload-progress-label').textContent = 'Ready';
  }

  async function upload() {
    if (busy) return;
    const file = state.displayFile;
    if (!file) return toast('Choose an image or GIF first', '', true);
    const gif = imageIsGif();
    if (gif && !$('#display-delay').reportValidity()) return;
    const layer = $('#display-layer').value;
    const query = new URLSearchParams({layer});
    if (gif && $('#display-delay').value) query.set('delay', $('#display-delay').value);
    const controls = $$('input,select,button', $('.display-studio'));
    const disabled = controls.map(input => input.disabled);
    busy = true;
    controls.forEach(input => input.disabled = true);
    const timer = setInterval(pollProgress, 250);
    try {
      await stopLive();
      const result = await withBusy($('#upload-display'), () => api(`/api/display?${query}`, {
        method:'POST',
        headers:{'Content-Type':'application/octet-stream', 'X-Filename':encodeURIComponent(file.name)},
        body:file
      }));
      toast('Display updated', result.frames > 1 ? `GIF · ${result.frames} frames · ${result.delay} ms` : `Slot ${+layer + 1}`);
    } catch (error) { toast('Upload failed', error.message, true); }
    finally {
      clearInterval(timer);
      await pollProgress();
      busy = false;
      controls.forEach((input, index) => input.disabled = disabled[index]);
      $('#upload-display').disabled = !state.displayFile;
    }
  }

  window.k8Display = {isBusy: () => busy, selectImage};
  $('#upload-display').addEventListener('click', upload);
  selectImage();
})();
