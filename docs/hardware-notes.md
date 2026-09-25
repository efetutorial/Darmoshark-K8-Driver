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
