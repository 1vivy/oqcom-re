# oqcom-re

A recovery environment for UEFI systems.

`oqcom-re.efi` is a std UEFI application built on uefi-rs. It lists every
formset in the HII database, shows them as a setup page, writes changed values
back through the HII configuration protocols and returns to its caller. Stock
firmware pages (for example the OVMF Device Manager) and the pages that a
platform publishes sit in the same list, because HII registration is additive.

The model, the HII provider, the widget and the GOP backend come from the
[`forms`](https://github.com/1vivy/forms) repository, pinned by revision in
`Cargo.toml`. This repository holds the application: display selection, key
input, the text-console fallback and the markers.

## Build

```sh
cargo build --release --target aarch64-unknown-uefi
cargo build --release --target x86_64-unknown-uefi
```

The artifacts are `out/cargo/<target>/release/oqcom-re.efi`.

`rust-toolchain.toml` pins `nightly-2026-08-08`, because `std::os::uefi::env`
is behind the unstable `uefi_std` gate (rust-lang/rust#100499). The same
standard library emits a `wcslen` libcall; the application defines it, because
UEFI has no C runtime.

## Display and keys

The application draws with `ratatui-gop` when the console handle publishes GOP,
and with the firmware text console otherwise.

| Key | Action |
| --- | --- |
| Up, Down (Qualcomm volume keys) | Move the selection |
| Enter (Qualcomm power key, scan `0x0102`) | Select, or commit an editor |
| Esc | Back, or cancel an editor |
| Left, Right, Home, Backspace, characters | Edit a value |

## Markers

The application writes these lines to the serial port. It also writes them to
the console, except while the session draws on the screen:

| Marker | Meaning |
| --- | --- |
| `OQCOM_RE_FORMSET title=<title>` | One line per formset found at start |
| `OQCOM_RE_READY formsets=<n>` | The first frame is drawn |
| `OQCOM_RE_EXIT` | The session ended; the application returns to its caller |
| `OQCOM_RE_ERROR reason=<reason>` | A step failed |
| `OQCOM_RE_FOCUS formset=<GUID>` | The load options named this formset; the session opened it |

## Load options

`--formset <GUID>` opens one formset directly and pins it, as
`FormBrowser2.SendForm` does: Back on its entry form ends the application. A
firmware owner uses it for one question, for example a consent prompt. When no
formset with that GUID is published, the application logs `OQCOM_RE_ERROR` and
`OQCOM_RE_EXIT` and returns at once, instead of showing the list.

## Test on QEMU

```sh
mkdir -p esp/EFI/BOOT
cp out/cargo/x86_64-unknown-uefi/release/oqcom-re.efi esp/EFI/BOOT/BOOTX64.EFI
cp /usr/share/edk2/x64/OVMF_VARS.4m.fd vars.fd
qemu-system-x86_64 -machine q35 -m 512 \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd \
    -drive if=pflash,format=raw,file=vars.fd \
    -drive format=raw,file=fat:rw:esp \
    -serial stdio -display none
```

OVMF publishes its own setup pages, so `OQCOM_RE_READY` reports more than zero
formsets.
