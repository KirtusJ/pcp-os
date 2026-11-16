"""Source-only, non-authorizing model for proposed PCP-OS installer target preflight.

NO DEVICE ACCESS. NO DISCOVERY. NO PERMISSION GRANT. NO INSTALLER INTEGRATION.
The caller supplies all observations. The code can only identify disagreements among
those claims and deliberately returns no 'authorized' or 'safe to write' result.
"""
from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Sequence

MAX_U64 = (1 << 64) - 1
VIRTIO_SECTOR_BYTES = 512  # Virtio block command LBA addressing unit.
SUPPORTED_GPT_LBA_BYTES = frozenset((512, 4096))  # Proposed first-device tier.
FIRST_TRANSPORT = "legacy-virtio-blk-pci"


class MediaCondition(str, Enum):
    FULL_ZERO_SCAN_REPORTED = "full-zero-scan-reported"
    GPT_VALID_REPORTED = "gpt-valid-reported"
    GPT_DAMAGED_REPORTED = "gpt-damaged-reported"
    UNKNOWN = "unknown"


class PreflightState(str, Enum):
    HOLD = "HOLD"
    ELIGIBLE_FOR_HOST_RECHECK = "ELIGIBLE_FOR_HOST_RECHECK"


@dataclass(frozen=True)
class ObservedDevice:
    # These are UNTRUSTED fixture fields in this model; the real OS must produce
    # and protect comparable observations in its own device enumeration path.
    handle: str
    generation: int
    transport: str
    bus_path: str
    capacity_512: int
    gpt_lba_bytes: int
    volume_marker: str
    uefi_media_id: int  # UEFI Block I/O UINT32 media-change value, NOT on-disk ID
    preflight_digest: str
    condition: MediaCondition
    read_only: bool = False
    flush_supported: bool = True
    present: bool = True
    outstanding_io: bool = False


@dataclass(frozen=True)
class Inventory:
    epoch: int
    devices: tuple[ObservedDevice, ...]


@dataclass(frozen=True)
class FreshInstallPlan:
    # An exact preflight comparison, NOT permission or an authenticated receipt.
    inventory_epoch: int
    handle: str
    generation: int
    transport: str
    bus_path: str
    capacity_512: int
    gpt_lba_bytes: int
    volume_marker: str
    uefi_media_id: int  # UEFI Block I/O UINT32 media-change value, NOT on-disk ID
    preflight_digest: str
    image_bytes: int
    image_sha256: str


@dataclass(frozen=True)
class PreflightResult:
    state: PreflightState
    reason: str


def _hold(reason: str) -> PreflightResult:
    return PreflightResult(PreflightState.HOLD, reason)


def _digest_looks_sha256(value: str) -> bool:
    return len(value) == 64 and all(x in "0123456789abcdef" for x in value)


def assess_fresh_install(inventory: Inventory, plan: FreshInstallPlan) -> PreflightResult:
    """Only check consistency of *claims*; never attest a real disk or grant a write.

    This conservative first-tier policy requires exactly one eligible writable
    transport, a fully-zero *reported* scan, full-size image geometry, and a
    source-bound digest. Installation, resume and upgrade need separate gates.
    """
    if inventory.epoch < 1 or plan.inventory_epoch != inventory.epoch:
        return _hold("missing-or-stale-inventory-epoch")
    if not _digest_looks_sha256(plan.image_sha256):
        return _hold("missing-or-malformed-image-digest")
    if plan.image_bytes <= 0 or plan.image_bytes > MAX_U64:
        return _hold("invalid-or-overflowed-image-length")
    if not inventory.devices:
        return _hold("no-observed-block-devices")
    if any((not d.handle or not d.bus_path or d.generation < 1 or
            d.capacity_512 <= 0 or d.capacity_512 > MAX_U64 or
            type(d.uefi_media_id) is not int or not (0 <= d.uefi_media_id < (1 << 32)))
           for d in inventory.devices):
        return _hold("incomplete-or-invalid-device-observation")
    # Duplicate handles/paths are an ambiguous enumeration, even if identifiers
    # differ; duplicate media IDs remain ambiguous even when paths differ.
    handles = [d.handle for d in inventory.devices if d.present]
    paths = [d.bus_path for d in inventory.devices if d.present]
    volume_markers = [d.volume_marker for d in inventory.devices if d.present and d.volume_marker]
    if len(handles) != len(set(handles)) or len(paths) != len(set(paths)):
        return _hold("duplicate-device-handle-or-path")
    if len(volume_markers) != len(set(volume_markers)):
        return _hold("duplicate-media-id")
    # A second writable block device is ambiguous for the initial narrow
    # installer profile even if its transport is currently unsupported.
    candidates = [d for d in inventory.devices if d.present and not d.read_only]
    if len(candidates) != 1:
        return _hold("first-tier-requires-one-writable-target")
    selected = [d for d in inventory.devices if d.handle == plan.handle]
    if len(selected) != 1:
        return _hold("named-handle-not-unique")
    d = selected[0]
    if not d.present or d.read_only:
        return _hold("selected-device-absent-or-read-only")
    if d.outstanding_io:
        return _hold("unfenced-outstanding-io")
    if d.transport != FIRST_TRANSPORT or d.transport != plan.transport:
        return _hold("transport-not-supported-or-changed")
    if d != candidates[0]:
        return _hold("selection-not-only-writable-supported-target")
    if d.generation != plan.generation or d.bus_path != plan.bus_path:
        return _hold("device-generation-or-path-changed")
    # UEFI MediaId is a firmware-provided change token, not a disk GUID.
    # It is meaningful only for the UEFI Block I/O transport that produced it.
    if type(plan.uefi_media_id) is not int or d.uefi_media_id != plan.uefi_media_id:
        return _hold("uefi-media-id-changed-or-invalid")
    if d.capacity_512 != plan.capacity_512 or d.gpt_lba_bytes != plan.gpt_lba_bytes:
        return _hold("device-geometry-changed")
    if d.gpt_lba_bytes not in SUPPORTED_GPT_LBA_BYTES:
        return _hold("gpt-lba-size-unsupported")
    if d.capacity_512 > MAX_U64 // VIRTIO_SECTOR_BYTES:
        return _hold("capacity-byte-overflow")
    capacity_bytes = d.capacity_512 * VIRTIO_SECTOR_BYTES
    if capacity_bytes % d.gpt_lba_bytes:
        return _hold("capacity-not-gpt-lba-aligned")
    if capacity_bytes != plan.image_bytes:
        return _hold("factory-image-capacity-not-exact")
    if not d.flush_supported:
        return _hold("durable-install-flush-not-advertised")
    if d.condition != MediaCondition.FULL_ZERO_SCAN_REPORTED:
        return _hold("target-not-reported-fully-blank")
    # A virgin blank disk has no on-media marker. Do not demand one as a
    # prerequisite for fresh setup; a marker alone never grants authority.
    # This is distinct from UEFI MediaId, which is a firmware change token.
    if d.volume_marker != plan.volume_marker:
        return _hold("media-id-mismatched")
    if (not _digest_looks_sha256(d.preflight_digest) or
            d.preflight_digest != plan.preflight_digest):
        return _hold("preflight-digest-missing-or-mismatched")
    # Intentionally not 'SAFE' or 'AUTHORIZED'. Caller could forge identical
    # observations for a clone. A real authority checker must bind trusted
    # enumerated handles, actual consent, current rights and current epoch.
    return PreflightResult(PreflightState.ELIGIBLE_FOR_HOST_RECHECK,
                           "claims-consistent-only-no-authority-granted")


class SourceStatus(str, Enum):
    INSPECTED_LOCAL = "INSPECTED_LOCAL"
    INSPECTED_EXTERNAL = "INSPECTED_EXTERNAL"
    NOT_INSPECTED = "NOT_INSPECTED"
    UNAVAILABLE = "UNAVAILABLE"


def classify_source(*, location: str, available: bool, inspected: bool,
                    networking_enabled: bool) -> SourceStatus:
    """Proposed *descriptive* provenance label, NOT a canonical ACR setting.

    Availability alone is never evidence. A network-off condition does not erase
    earlier genuinely inspected external source bytes, if inspection is known.
    """
    if location not in ("local", "external"):
        raise ValueError("location must be local or external")
    if inspected and not available:
        raise ValueError("inspection cannot be asserted for an unavailable source")
    if inspected:
        return (SourceStatus.INSPECTED_LOCAL if location == "local"
                else SourceStatus.INSPECTED_EXTERNAL)
    # Accessible cached source bytes can remain unread while offline.
    # Network state does not alter whether the source is available locally.
    if not available:
        return SourceStatus.UNAVAILABLE
    return SourceStatus.NOT_INSPECTED
