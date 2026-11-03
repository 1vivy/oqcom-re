# oqcom-re

A recovery environment for UEFI systems.

`oqcom-re.efi` is a std UEFI application built on uefi-rs. It lists every
formset in the HII database, shows them as a setup page, writes changed values
back through the HII configuration protocols and returns to its caller. Stock
firmware pages (for example the OVMF Device Manager) and the pages that a
platform publishes sit in the same list, because HII registration is additive.

The application is a screen and input owner, not a transport owner. A caller
can publish one optional protocol before it starts the application to take the
screen back for its own host work, for example a native fastboot session.

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
Input uses the inherited `ConIn` handle and interface, not the first global
keyboard protocol. Output uses the inherited `ConOut` handle.

The graphics grid starts below the top edge of the panel and ends four physical
pixels above its bottom edge. The top margin is 46/1000 of the live mode height,
which is 127 pixels on the 2772-pixel panel, so the application stores no fixed
panel geometry. The application supplies these margins to the graphics backend
in physical pixels.

Navigation redraws the page without re-enumerating HII. A provider's Refresh
response reloads its formsets. A provider's Exit response ends the application
so the lifecycle can execute the selected action.

Up and Down wrap at both ends of the rows, and a choice editor ends with a
`< Back` row. A phone with volume and power keys alone reaches every row and
cancels an editor without an Esc key.

| Key | Action |
| --- | --- |
| Up, Down (Qualcomm volume keys) | Move the selection |
| Enter (Qualcomm power key, scan `0x0102`) | Select, or commit an editor |
| Esc | Back, or cancel an editor |
| Left, Right, Home, Backspace, characters | Edit a value |

## Markers

The application writes these lines to the serial port when firmware publishes
`SerialIo`. Otherwise it uses the console and repaints any disturbed frame.
With a separate serial sink, console markers stop while the session draws.

| Marker | Meaning |
| --- | --- |
| `OQCOM_RE_FORMSET title=<title>` | One line per formset found at start |
| `OQCOM_RE_READY formsets=<n>` | The first frame is drawn |
| `OQCOM_RE_HOST_YIELD` | The caller asked for the screen back; the session ends |
| `OQCOM_RE_EXIT` | The session ended; the application returns to its caller |
| `OQCOM_RE_ERROR reason=<reason>` | A step failed |
| `OQCOM_RE_FOCUS formset=<GUID>` | The load options named this formset; the session opened it |

## Load options

`--formset <GUID>` opens one formset directly and pins it, as
`FormBrowser2.SendForm` does: Back on its entry form ends the application. A
firmware owner uses it for one question, for example a consent prompt. When no
formset with that GUID is published, the application logs `OQCOM_RE_ERROR` and
`OQCOM_RE_EXIT` and returns at once, instead of showing the list.

## Host protocol

A caller can publish one cooperative protocol before it starts the application.
The application looks it up once at startup and polls it at the top of every
input-loop turn, also while no key waits (one idle turn waits 10 ms). When the
protocol asks for the screen, the application logs `OQCOM_RE_HOST_YIELD`, writes
`OQCOM_RE_EXIT` and returns to its caller without acting on the selected row.
It draws only after a key stroke, so an idle session does not redraw.

The interface lives in [`crates/oqcom-re-host`](crates/oqcom-re-host). It is
dependency-free and compiles on stable Rust 1.95, so another firmware project
adds it as a git dependency:

```toml
oqcom-re-host = { git = "https://github.com/1vivy/oqcom-re", rev = "<revision>" }
```

| Item | Value |
| --- | --- |
| Protocol GUID | `34bf6d61-c86c-4e92-92b6-5937d468ca2d` |
| Revision | `1` |
| Request `YIELD_TO_HOST` | `1`; the application returns at once |
| Request `0` or unknown | The application keeps the screen |

- The caller installs the interface before `StartImage` and keeps it alive
  until the application returns.
- `poll` runs at `TPL_APPLICATION` only, never from a timer notification, and
  writes the request word on every supported path.
- A status other than zero, or a revision other than `1`, is reported as
  `OQCOM_RE_ERROR` and the session ends. The application does not claim host
  access that the caller did not grant.
- Without the protocol the application runs alone, and `--formset` behaves as
  before.

The application knows no host transport facts: no USB, fastboot or Qualcomm
code lives here. The caller owns the transport.

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
