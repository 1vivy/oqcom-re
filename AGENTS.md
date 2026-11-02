# Working in oqcom-re

Read [README.md](README.md) first.

## Commands

```sh
cargo fmt --check
cargo clippy --workspace --target x86_64-unknown-uefi --all-targets -- -D warnings
cargo clippy --workspace --target aarch64-unknown-uefi --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release --target aarch64-unknown-uefi
cargo build --workspace --release --target x86_64-unknown-uefi
cargo +1.95.0 clippy -p oqcom-re-host --all-targets -- -D warnings
cargo +1.95.0 test -p oqcom-re-host
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
- `crates/oqcom-re-host` is the only place that names the host GUID, the
  revision or the request values. Keep it dependency-free and on stable Rust
  1.95: another firmware project adds it as a git dependency. The application
  keeps its nightly pin.
- The application polls the optional protocol at the top of every input-loop
  turn, before it reads a key stroke, and draws only after a key stroke. A
  `YIELD_TO_HOST` request logs `OQCOM_RE_HOST_YIELD` and ends the session
  without acting on the selected row. No idle turn redraws.
- The application knows no host transport facts. No USB, fastboot or Qualcomm
  code belongs here; the caller owns the transport.
- No protocol means standalone operation. A wrong revision or a failed poll is
  reported as `OQCOM_RE_ERROR` and ends the session; never continue as if the
  caller had granted host access.

## Toolchain

`nightly-2026-08-08`, for the `uefi_std` gate. The `forms` libraries stay on
stable in their own repository; do not add a nightly feature to them.
`crates/oqcom-re-host` stays on stable Rust 1.95 and has no dependencies,
because a firmware project builds it as a git dependency.

## Evidence

A green build is not a firmware test. A QEMU or device run reports the firmware
file, the marker lines from the serial log, and what was not exercised.
Write prose in short, plain sentences (ASD-STE100 style).
