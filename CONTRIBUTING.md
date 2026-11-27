# Contributing

Use stable Rust and keep user-facing text in English. Before opening a pull
request, run:

```sh
cargo fmt -- --check
cargo test --locked
```

Do not commit build output, captured device traffic containing personal data, or
manufacturer binaries. Hardware protocol changes should include the device model,
firmware version and a reproducible observation in `docs/hardware-notes.md`.
