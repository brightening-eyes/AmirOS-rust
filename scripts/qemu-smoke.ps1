# QEMU smoke test (Windows/local variant of scripts/qemu-smoke.sh).
# Boots the kernel ISO headlessly and asserts that an expected serial-output
# marker is printed within the timeout.
#
# Usage: powershell -File scripts/qemu-smoke.ps1 [-Profile release] [-TimeoutSec 90]
# Requires on PATH: xorriso, qemu-system-x86_64 (and limine\limine.exe from the submodule)
[CmdletBinding()]
param(
	[string]$Target = "x86_64-unknown-none",
	[string]$Profile = "release",
	[int]$TimeoutSec = 90,
	[string]$Marker = "allocator initialized"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "target\$Target\$Profile\amir_os"
if (-not (Test-Path -LiteralPath $bin)) {
	Write-Error "kernel not found at $bin — run 'cargo build --release' first"
}
foreach ($tool in @("xorriso", "qemu-system-x86_64")) {
	if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
		Write-Error "$tool is required on PATH"
	}
}
$limineExe = Join-Path $root "limine\limine.exe"
if (-not (Test-Path -LiteralPath $limineExe)) {
	Write-Error "limine host tool not found at $limineExe"
}

$work = New-Item -ItemType Directory -Path (Join-Path ([IO.Path]::GetTempPath()) ("amiros-smoke-" + [guid]::NewGuid().ToString("N")))
try {
	$isoRoot = Join-Path $work.FullName "iso_root"
	New-Item -ItemType Directory -Force -Path (Join-Path $isoRoot "boot\limine") | Out-Null
	Copy-Item $bin (Join-Path $isoRoot "boot\amir_os")
	Copy-Item (Join-Path $root "limine.conf") (Join-Path $isoRoot "boot\")
	Copy-Item (Join-Path $root "limine\limine-bios.sys") (Join-Path $isoRoot "boot\limine\")
	Copy-Item (Join-Path $root "limine\limine-bios-cd.bin") (Join-Path $isoRoot "boot\limine\")
	Copy-Item (Join-Path $root "limine\limine-uefi-cd.bin") (Join-Path $isoRoot "boot\limine\")

	Write-Host "== assembling boot ISO =="
	$iso = Join-Path $work.FullName "amir-smoke.iso"
	& xorriso -as mkisofs -R -r -J -b boot/limine/limine-bios-cd.bin `
		-no-emul-boot -boot-load-size 4 -boot-info-table -hfsplus `
		-apm-block-size 2048 --efi-boot boot/limine/limine-uefi-cd.bin `
		-efi-boot-part --efi-boot-image --protective-msdos-label `
		"$isoRoot" -o "$iso" 2>&1 | Out-Null
	if ($LASTEXITCODE -ne 0) { Write-Error "xorriso failed with exit code $LASTEXITCODE" }
	& $limineExe bios-install $iso | Out-Null
	if ($LASTEXITCODE -ne 0) { Write-Error "limine bios-install failed" }

	Write-Host "== booting in QEMU (waiting up to ${TimeoutSec}s for '$Marker') =="
	$log = Join-Path $work.FullName "serial.log"
	$qemuArgs = @(
		"-M", "q35", "-m", "2G", "-smp", "1",
		"-display", "none", "-no-reboot", "-no-shutdown",
		"-serial", "file:$log", "-cdrom", $iso
	)
	$qemu = Start-Process -FilePath "qemu-system-x86_64" -ArgumentList $qemuArgs -PassThru
	$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSec)
	$found = $false
	while ([DateTime]::UtcNow -lt $deadline) {
		Start-Sleep -Milliseconds 500
		if ((Test-Path -LiteralPath $log) -and (Select-String -LiteralPath $log -Pattern $Marker -Quiet)) {
			$found = $true
			break
		}
		if ($qemu.HasExited) { break }
	}
	try { if (-not $qemu.HasExited) { Stop-Process -Id $qemu.Id -Force } } catch {}

	if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log }
	if ($found) {
		Write-Host "smoke PASSED: '$Marker' observed on serial"
		exit 0
	} else {
		Write-Error "smoke FAILED: '$Marker' not observed within ${TimeoutSec}s"
	}
} finally {
	Remove-Item -Recurse -Force $work.FullName -ErrorAction SilentlyContinue
}
