#!/usr/bin/env bash
# QEMU smoke test: boots the kernel ISO headlessly and asserts that an
# expected serial-output marker is printed within the timeout.
#
# Usage: scripts/qemu-smoke.sh [target] [profile] [timeout_secs]
# Requires on PATH: xorriso, qemu-system-x86_64, cc (for the limine host tool)
set -euo pipefail

TARGET="${1:-x86_64-unknown-none}"
PROFILE="${2:-release}"
TIMEOUT_SECS="${3:-90}"
MARKER="${SMOKE_MARKER:-allocator initialized}"

root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$root/target/$TARGET/$PROFILE/amir_os"
if [[ ! -f "$bin" ]]; then
	echo "error: kernel not found at $bin — run 'cargo build --release' first" >&2
	exit 1
fi
command -v xorriso >/dev/null || { echo "error: xorriso is required" >&2; exit 1; }
command -v "qemu-system-x86_64" >/dev/null || { echo "error: qemu-system-x86_64 is required" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/iso_root/boot/limine"
cp "$bin" "$work/iso_root/boot/amir_os"
cp "$root/limine.conf" "$work/iso_root/boot/"
cp "$root/limine/limine-bios.sys" "$work/iso_root/boot/limine/"
cp "$root/limine/limine-bios-cd.bin" "$work/iso_root/boot/limine/"
cp "$root/limine/limine-uefi-cd.bin" "$work/iso_root/boot/limine/"

echo "== building limine host tool =="
limine_tool="$work/limine"
cc -O2 -std=gnu11 -o "$limine_tool" "$root/limine/limine.c"

echo "== assembling boot ISO =="
iso="$work/amir-smoke.iso"
xorriso -as mkisofs -R -r -J -b boot/limine/limine-bios-cd.bin \
	-no-emul-boot -boot-load-size 4 -boot-info-table -hfsplus \
	-apm-block-size 2048 --efi-boot boot/limine/limine-uefi-cd.bin \
	-efi-boot-part --efi-boot-image --protective-msdos-label \
	"$work/iso_root" -o "$iso" >/dev/null 2>&1
"$limine_tool" bios-install "$iso" >/dev/null

echo "== booting in QEMU (waiting up to ${TIMEOUT_SECS}s for '$MARKER') =="
log="$work/serial.log"
set +e
timeout --foreground "$TIMEOUT_SECS" \
	qemu-system-x86_64 -M q35 -m 2G -smp 1 -display none \
	-no-reboot -no-shutdown -serial stdio \
	-cdrom "$iso" >"$log" 2>&1
qemu_rc=$?
set -e

cat "$log"
echo "== serial end (qemu exit code: $qemu_rc) =="

if grep -q "$MARKER" "$log"; then
	echo "smoke PASSED: '$MARKER' observed on serial"
	exit 0
else
	echo "smoke FAILED: '$MARKER' not observed within ${TIMEOUT_SECS}s" >&2
	exit 1
fi
