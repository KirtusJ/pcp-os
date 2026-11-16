"""Fixture-only negative controls. Never opens a block device or writes bytes."""
from dataclasses import replace
import unittest

from target_preflight import (
    FIRST_TRANSPORT, Inventory, FreshInstallPlan, MediaCondition,
    ObservedDevice, PreflightState, SourceStatus, assess_fresh_install,
    classify_source,
)

CAP = 2048  # exactly 1 MiB, all fake; no allocated disk image.
ID = "toy-id-a"  # deliberately non-authenticating
HASH = "a" * 64
IMAGE = "b" * 64


def example():
    d = ObservedDevice(
        handle="host-minted-handle-A", generation=3,
        transport=FIRST_TRANSPORT, bus_path="pci/0000:00:05.0",
        capacity_512=CAP, gpt_lba_bytes=512, media_id=ID,
        preflight_digest=HASH,
        condition=MediaCondition.FULL_ZERO_SCAN_REPORTED,
    )
    p = FreshInstallPlan(
        inventory_epoch=9, handle=d.handle, generation=d.generation,
        transport=d.transport, bus_path=d.bus_path,
        capacity_512=d.capacity_512, gpt_lba_bytes=d.gpt_lba_bytes,
        media_id=d.media_id, preflight_digest=d.preflight_digest,
        image_bytes=CAP * 512, image_sha256=IMAGE,
    )
    return Inventory(epoch=9, devices=(d,)), p


class PreflightTests(unittest.TestCase):
    def assert_hold(self, inventory, plan):
        self.assertEqual(assess_fresh_install(inventory, plan).state,
                         PreflightState.HOLD)

    def test_consistent_claims_are_only_eligible_for_further_host_check(self):
        inv, plan = example()
        result = assess_fresh_install(inv, plan)
        self.assertEqual(result.state, PreflightState.ELIGIBLE_FOR_HOST_RECHECK)
        self.assertNotIn("AUTHORIZED", result.state.value)

    def test_4096_gpt_logical_sector_policy(self):
        inv, plan = example()
        d = replace(inv.devices[0], gpt_lba_bytes=4096)
        self.assertEqual(assess_fresh_install(replace(inv, devices=(d,)),
                          replace(plan, gpt_lba_bytes=4096)).state,
                         PreflightState.ELIGIBLE_FOR_HOST_RECHECK)

    def test_stale_epoch(self):
        inv, p = example()
        self.assert_hold(replace(inv, epoch=10), p)

    def test_generation_changed(self):
        inv, p = example()
        self.assert_hold(replace(inv, devices=(replace(inv.devices[0],generation=4),)), p)

    def test_bus_path_swapped_even_if_media_id_is_cloned(self):
        inv, p = example()
        self.assert_hold(replace(inv, devices=(replace(inv.devices[0],bus_path="pci/0000:00:06.0"),)), p)

    def test_same_device_id_on_two_present_drives(self):
        inv, p = example()
        clone = replace(inv.devices[0], handle="different-handle",bus_path="pci/0000:00:06.0")
        self.assert_hold(replace(inv, devices=(inv.devices[0],clone)), p)

    def test_another_writable_device_even_with_different_id(self):
        inv, p = example()
        extra = replace(inv.devices[0], handle="different-handle",bus_path="pci/0000:00:06.0",media_id="different-id")
        self.assert_hold(replace(inv, devices=(inv.devices[0],extra)), p)

    def test_other_read_only_device_does_not_create_two_writable_targets(self):
        inv, p = example()
        other = replace(inv.devices[0], handle="different-handle",bus_path="pci/0000:00:06.0",media_id="different-id",read_only=True)
        self.assertEqual(assess_fresh_install(replace(inv,devices=(inv.devices[0],other)),p).state,
                         PreflightState.ELIGIBLE_FOR_HOST_RECHECK)

    def test_duplicate_bus_path_is_ambiguous(self):
        inv, p = example()
        extra = replace(inv.devices[0],handle="different-handle",media_id="different-id",read_only=True)
        self.assert_hold(replace(inv,devices=(inv.devices[0],extra)),p)

    def test_target_absent(self):
        inv, p = example()
        self.assert_hold(replace(inv, devices=(replace(inv.devices[0],present=False),)),p)

    def test_target_read_only(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],read_only=True),)),p)

    def test_outstanding_io_fence(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],outstanding_io=True),)),p)

    def test_transport_not_other_ide_or_ahci(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],transport="ahci"),)),p)

    def test_capacity_changed(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],capacity_512=CAP+1),)),p)

    def test_small_device_prevents_partial_factory_image(self):
        inv, p = example()
        d = replace(inv.devices[0], capacity_512=CAP//2)
        self.assert_hold(replace(inv, devices=(d,)),replace(p,capacity_512=CAP//2))

    def test_large_device_rejects_stale_gpt_backup_lba(self):
        inv, p = example()
        d = replace(inv.devices[0], capacity_512=CAP*2)
        self.assert_hold(replace(inv,devices=(d,)),replace(p,capacity_512=CAP*2))

    def test_unsupported_gpt_2048_byte_target_lba(self):
        inv, p = example()
        d = replace(inv.devices[0],gpt_lba_bytes=2048)
        self.assert_hold(replace(inv,devices=(d,)),replace(p,gpt_lba_bytes=2048))

    def test_capacity_gpt_block_misaligned(self):
        inv, p = example()
        d = replace(inv.devices[0],capacity_512=CAP+1,gpt_lba_bytes=4096)
        self.assert_hold(replace(inv,devices=(d,)),replace(p,capacity_512=CAP+1,gpt_lba_bytes=4096,image_bytes=(CAP+1)*512))

    def test_overflowed_capacity(self):
        inv, p = example()
        d = replace(inv.devices[0], capacity_512=(1<<64)-1)
        self.assert_hold(replace(inv,devices=(d,)),replace(p,capacity_512=(1<<64)-1))

    def test_no_flush_durability_claim(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],flush_supported=False),)),p)

    def test_nonblank_valid_gpt_not_fresh_install(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],condition=MediaCondition.GPT_VALID_REPORTED),)),p)

    def test_damaged_gpt_requires_recovery_separate(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],condition=MediaCondition.GPT_DAMAGED_REPORTED),)),p)

    def test_unknown_media_not_silently_blank(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],condition=MediaCondition.UNKNOWN),)),p)

    def test_preflight_digest_changed(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],preflight_digest="c"*64),)),p)

    def test_media_id_changed(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],media_id="other"),)),p)

    def test_missing_image_digest(self):
        inv, p = example()
        self.assert_hold(inv,replace(p,image_sha256=""))

    def test_no_inventory(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=()),p)

    def test_incomplete_device_record(self):
        inv, p = example()
        self.assert_hold(replace(inv,devices=(replace(inv.devices[0],handle=""),)),p)

    def test_reordered_enumeration_is_not_identity(self):
        inv, p = example()
        ro=replace(inv.devices[0],handle="other",bus_path="pci/0000:00:09.0",media_id="different",read_only=True)
        self.assertEqual(assess_fresh_install(replace(inv,devices=(ro,inv.devices[0])),p).state,
                         PreflightState.ELIGIBLE_FOR_HOST_RECHECK)

    def test_limit_same_port_clone_is_indistinguishable_to_forged_facts(self):
        # An attacker replaces the disk while recreating EVERY presented field.
        # No model that only sees these fields can detect this; a fresh trusted
        # grant and observed hardware boundary are *still* necessary.
        inv,p=example()
        self.assertEqual(assess_fresh_install(inv,p).state,
                         PreflightState.ELIGIBLE_FOR_HOST_RECHECK)


class OfflineEvidenceTests(unittest.TestCase):
    def test_offline_local_source_actually_inspected(self):
        self.assertEqual(classify_source(location="local",available=True,
                         inspected=True,networking_enabled=False),SourceStatus.INSPECTED_LOCAL)

    def test_offline_local_source_absent(self):
        self.assertEqual(classify_source(location="local",available=False,
                         inspected=False,networking_enabled=False),SourceStatus.UNAVAILABLE)

    def test_offline_external_unavailable(self):
        self.assertEqual(classify_source(location="external",available=False,
                         inspected=False,networking_enabled=False),SourceStatus.UNAVAILABLE)

    def test_online_external_source_available_but_unread(self):
        self.assertEqual(classify_source(location="external",available=True,
                         inspected=False,networking_enabled=True),SourceStatus.NOT_INSPECTED)

    def test_external_source_inspected_before_network_disabled(self):
        self.assertEqual(classify_source(location="external",available=True,
                         inspected=True,networking_enabled=False),SourceStatus.INSPECTED_EXTERNAL)

    def test_cannot_claim_unavailable_source_inspected(self):
        with self.assertRaises(ValueError):
            classify_source(location="external",available=False,
                            inspected=True,networking_enabled=False)


if __name__ == "__main__":
    unittest.main()
