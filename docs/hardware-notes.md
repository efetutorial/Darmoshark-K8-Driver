# K8 hardware checks — 2026-09-24

## USB report rate

The official 1.2.69 driver declares 125/250/500/1000 Hz for the K8's
`yc200CommonLayoutFour`. The report setter uses command `04`, profile in byte 1,
and interval code 1/2/4/8 in byte 2. Readback uses `84`.

On the attached 3151:4015 keyboard (firmware 257, profile 0), requesting 125 Hz
returned 125 Hz; requesting 500 Hz returned 500 Hz. The original 1000 Hz setting
was restored and read back successfully. Its USB full-speed interrupt endpoints
advertise a 1 ms interval.

This verifies writable firmware configuration, not measured key-to-host latency.
The host's USB polling schedule and the device's production of input reports are
separate. No USB input timing or physical switch-to-host benchmark was performed.
Keep 1000 Hz for the shortest configured report interval; a lower value is not a
latency improvement. The UI verifies readback after writes and reads the selected
profile's value rather than always reading profile 0.

USB interval reference: [Linux USB host API](https://www.kernel.org/doc/html/v6.12/driver-api/usb/usb.html).

## LCD

The K8's official `yc200LEDOtherH128Date` definition specifies 128×128 RGB565,
five image slots, and native date/time support. Image transfer uses handshake
`A5` and 56-byte pixel chunks in command `25`. The native clock uses `28` with
a big-endian year and month/day/hour/minute/second at bytes 8–14.

Existing image/GIF conversion and packet tests pass. New generated cards use the
same RGB565 encoder and transfer path, accepting exactly one 128×128 RGBA frame.
No test image was written over the user's current LCD content during this change,
and the physical display's appearance was not visually verified.

A full raster needs 586 pixel packets plus the handshake. The driver's explicit
1 ms spacing alone is approximately 0.6 seconds, before HID overhead. No separate,
verified RAM-only live framebuffer transport was found in the official K8 path.
Continuous mirroring is therefore not offered by repeatedly uploading stored
images. The app offers **one-shot screen snapshots** instead.

Native Wayland screenshot capture uses `grim` and was exercised successfully.
Browser capture stops all media tracks immediately after acquiring one frame.
Captured images are previewed first and are only sent to the LCD after Upload.
The captured image stays in memory and is resized before preview. Browsers with
getDisplayMedia can use their screen picker.

Info cards support text, date/time, and memory/uptime snapshots. They only refresh
on user action. Use the built-in clock for continuously advancing time without
background image uploads. Syncing time does not switch the LCD's active screen;
choose the clock with the keyboard's own controls.

## Superseded single-photo experiment — 2026-10-24

The K8 (`3151:4015`, vendor usage `ffff:0002`) can retain an old animation
alongside its five still-image slots. Updating one still slot does not replace
the other slots or the old GIF.

The locally available official `Darmoshark_setup_2.1.66(WIN20240723).exe`
identifies the K8 as `yc500_k8_soc`. Its RGB565 uploader
(`getTFTLCDDataRGBImg` / `setTFTLCDDataRGBImg`) uses `A5` / `25`,
the frame count at byte 2, and the current frame or still slot at byte 1.
It retries a busy announcement before sending pixels. These builders agree
with the existing K8 upload path above.

An initial **One photo everywhere** experiment sent two identical frames and
then wrote all still slots. This was based on an unverified assumption about
automatic picture cycling. It also silently selected a GIF's first frame by
default. The option has been removed; it is not the current upload workflow.

On the attached K8 (firmware 257, profile 0), back-to-back full-image uploads
without the vendor uploader's final 500 ms wait were rejected partway through
the still slots. Adding that wait allowed the complete sequence to finish:
the app's K8 icon was uploaded as two identical animation frames and to all
five still slots. Every announcement was accepted and every HID write
succeeded. Native Tauri startup and settings readback also succeeded.

Readback before and after the upload confirmed unchanged firmware, active
profile, report rate, lighting, keyboard options, sleep timers and debounce.
The panel itself was not visually inspected, and persistence across a power
cycle was not tested. No flash-erase, bootloader, or factory-reset command
was used by this option.

## GIF and individual-slot correction — 2026-10-25

GIF decoding now retains every frame up to the device limit. The firmware has
one frame delay, so auto timing uses the mean GIF delay instead of taking only
the first frame's delay. No first-frame or all-slot mode is selected implicitly.
Still images write only the selected slot; GIFs use the vendor animation path.

Uploads now send the full 128×128 raster, including black letterboxing, rather
than cropping to non-black bounds. This avoids relying on cropped-region
replacement to clear pixels from a previously stored image. Column-major
RGB565 and big-endian pixel bytes match the vendor encoder. Packet tests cover
asymmetric pixel placement, distinct animation frames, frame/page indices and
all five still-slot addresses. The 500 ms commit wait is retained.

Image preparation is reported before decoding starts and errors reset the
progress state. Native deployment uses an optimized release build.

A test GIF (896×1280, 100 frames) was decoded to
100 distinct 128×128 RGB565 frames with a 67 ms mean frame delay. Optimized
preparation took approximately 1.3 seconds. On firmware 257 the entire
100-frame transfer completed, including every full-size HID write. Settings
readback before and after the transfer confirmed unchanged profile, RGB,
report rate, options, sleep timers and debounce. Physical appearance still
requires the user's observation; transfer completion alone is not visual
verification.
