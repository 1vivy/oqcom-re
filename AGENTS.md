# Working in oqcom-re

Read [README.md](README.md) first.

## Commands

```sh
cargo fmt --check
cargo clippy --target x86_64-unknown-uefi --all-targets -- -D warnings
cargo clippy --target aarch64-unknown-uefi --all-targets -- -D warnings
cargo test
cargo build --release --target aarch64-unknown-uefi
cargo build --release --target x86_64-unknown-uefi
```

## Boundaries

- The forms model, the HII provider, the widget and the GOP backend belong to
  the `forms` repository. Change them there, then move the revision pin here.
- This repository holds only what the application needs: startup, display
  selection, key input, the text-console backend and the markers.
- By default, the application lists every HII formset without a GUID or class filter.
- `--formset <GUID>` opens one formset. Back on its entry form returns to the caller.
- Keep the visible Back row available for phones with only volume and power keys.
- Returning from `main` returns to the caller of `StartImage`. The application
  does not reset the system and does not exit boot services; a provider
  `Response::Reset` is shown to the person.

## Toolchain

`nightly-2026-08-08`, for the `uefi_std` gate. The `forms` libraries stay on
stable in their own repository; do not add a nightly feature to them.

## Evidence

A green build is not a firmware test. A QEMU or device run reports the firmware
file, the marker lines from the serial log, and what was not exercised.
Write prose in short, plain sentences (ASD-STE100 style).
