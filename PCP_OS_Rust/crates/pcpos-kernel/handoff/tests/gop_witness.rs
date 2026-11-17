//! Synthetic observations. These tests neither invoke GOP nor verify an EDID.
//! A consistent record is not an authenticated firmware event.

use pcpos_boot_handoff::gop_witness::{
    assess, Assessment, Capture, Error, Initiator, ModeSnapshot, SetModeCall,
    SetModeStatus, Stage,
};

fn captured_mode_change() -> Capture {
    let before = ModeSnapshot {
        device_key: 7,
        sequence: 10,
        stage: Stage::BootServicesActive,
        current_mode: 1,
        max_modes: 5,
        width: 1280,
        height: 720,
        pixels_per_scan_line: 1280,
        pixel_format: 1,
        framebuffer_base: 0xf000_0000,
        framebuffer_size: 1280 * 720 * 4,
    };
    let after = ModeSnapshot {
        device_key: 7,
        sequence: 30,
        stage: Stage::BootServicesActive,
        current_mode: 3,
        max_modes: 5,
        width: 1920,
        height: 1080,
        pixels_per_scan_line: 2048,
        pixel_format: 1,
        framebuffer_base: 0x1_1000_0000,
        framebuffer_size: 2048 * 1080 * 4,
    };
    Capture {
        before,
        set_mode: Some(SetModeCall {
            device_key: 7,
            sequence: 20,
            initiator: Initiator::OsControlledPreExit,
            stage: Stage::BootServicesActive,
            requested_mode: 3,
            status: SetModeStatus::EfiSuccess,
        }),
        after,
        claimed_complete: true,
    }
}

#[test]
fn matching_pre_exit_os_setmode_with_distinct_readback_is_consistent() {
    assert_eq!(assess(&captured_mode_change()), Ok(Assessment::ConsistentWithOsModeChange));
}

#[test]
fn zero_os_owned_setmode_does_not_prove_os_chose_existing_preferred_mode() {
    let mut c = captured_mode_change();
    c.set_mode = None;
    c.after = c.before;
    c.after.sequence = 30;
    assert_eq!(assess(&c), Ok(Assessment::ReportedAlreadyActive));
}

#[test]
fn incomplete_trace_cannot_claim_inherited_mode_even_if_snapshots_match() {
    let mut c = captured_mode_change();
    c.set_mode = None;
    c.claimed_complete = false;
    c.after = c.before;
    c.after.sequence = 30;
    assert_eq!(assess(&c), Ok(Assessment::MissingCaptureCoverage));
}

#[test]
fn incomplete_trace_cannot_confirm_os_selection_despite_successful_looking_call() {
    let mut c = captured_mode_change();
    c.claimed_complete = false;
    assert_eq!(assess(&c), Ok(Assessment::MissingCaptureCoverage));
}

#[test]
fn no_recorded_call_with_changed_mode_must_not_be_attributed_to_os() {
    let mut c = captured_mode_change();
    c.set_mode = None;
    assert_eq!(assess(&c), Ok(Assessment::UnattributedChange));
}

#[test]
fn no_recorded_call_with_changed_geometry_must_not_be_called_already_active() {
    let mut c = captured_mode_change();
    c.set_mode = None;
    c.after.current_mode = c.before.current_mode;
    // Framebuffer size, geometry and pitch still differ.
    assert_eq!(assess(&c), Ok(Assessment::UnattributedChange));
}

#[test]
fn firmware_owned_setmode_is_not_os_selection() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().initiator = Initiator::Firmware;
    assert_eq!(assess(&c), Ok(Assessment::OtherActorModeChange));
}

#[test]
fn grub_owned_setmode_is_not_os_selection() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().initiator = Initiator::Bootloader;
    assert_eq!(assess(&c), Ok(Assessment::OtherActorModeChange));
}

#[test]
fn unknown_caller_is_not_implicitly_pcpos_owned() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().initiator = Initiator::Unattributed;
    assert_eq!(assess(&c), Ok(Assessment::UnattributedChange));
}

#[test]
fn native_driver_change_is_not_uefi_gop_setmode() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().initiator = Initiator::NativeDriver;
    assert_eq!(assess(&c), Err(Error::InvalidInitiator));
}

#[test]
fn repeated_setmode_on_same_mode_index_does_not_demonstrate_switch() {
    let mut c = captured_mode_change();
    c.after = c.before;
    c.after.sequence = 30;
    c.set_mode.as_mut().unwrap().requested_mode = c.before.current_mode;
    assert_eq!(assess(&c), Ok(Assessment::ReassertedActiveMode));
}

#[test]
fn successful_same_mode_number_with_changed_pitch_is_not_an_unchanged_reassertion() {
    let mut c = captured_mode_change();
    c.after = c.before;
    c.after.sequence = 30;
    c.after.pixels_per_scan_line = c.before.pixels_per_scan_line + 64;
    c.set_mode.as_mut().unwrap().requested_mode = c.before.current_mode;
    assert_eq!(assess(&c), Ok(Assessment::SameModeNumberChangedMetadata));
}

#[test]
fn successful_same_mode_number_with_new_framebuffer_base_is_reported_separately() {
    let mut c = captured_mode_change();
    c.after = c.before;
    c.after.sequence = 30;
    c.after.framebuffer_base = 0x1_1000_0000;
    c.set_mode.as_mut().unwrap().requested_mode = c.before.current_mode;
    assert_eq!(assess(&c), Ok(Assessment::SameModeNumberChangedMetadata));
}

#[test]
fn failed_call_with_same_mode_number_and_different_framebuffer_is_not_no_effect() {
    let mut c = captured_mode_change();
    c.after = c.before;
    c.after.sequence = 30;
    c.after.framebuffer_size += 4096;
    let call = c.set_mode.as_mut().unwrap();
    call.requested_mode = c.before.current_mode;
    call.status = SetModeStatus::EfiDeviceError;
    assert_eq!(assess(&c), Ok(Assessment::FailedCallChangedMetadata));
}

#[test]
fn failed_call_with_same_mode_number_and_changed_mode_catalog_is_not_no_effect() {
    let mut c = captured_mode_change();
    c.after = c.before;
    c.after.sequence = 30;
    c.after.max_modes += 1;
    let call = c.set_mode.as_mut().unwrap();
    call.requested_mode = c.before.current_mode;
    call.status = SetModeStatus::EfiUnsupported;
    assert_eq!(assess(&c), Ok(Assessment::FailedCallChangedMetadata));
}

#[test]
fn pixelbltonly_with_no_usable_scanline_can_be_a_selected_mode() {
    let mut c = captured_mode_change();
    c.after.pixel_format = 3; // GOP PixelBltOnly
    c.after.pixels_per_scan_line = 0; // never used for a direct framebuffer
    c.after.framebuffer_base = 0;
    c.after.framebuffer_size = 0;
    assert_eq!(assess(&c), Ok(Assessment::ConsistentWithOsModeChange));
}

#[test]
fn linear_pixel_format_does_not_allow_zero_scanline_pitch() {
    let mut c = captured_mode_change();
    c.after.pixel_format = 1;
    c.after.pixels_per_scan_line = 0;
    assert_eq!(assess(&c), Err(Error::InvalidGeometry));
}

#[test]
fn successful_call_without_matching_post_readback_is_contradictory() {
    let mut c = captured_mode_change();
    c.after.current_mode = 2;
    assert_eq!(assess(&c), Ok(Assessment::ContradictoryReadback));
}

#[test]
fn unsupported_mode_not_silently_recorded_as_success() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().status = SetModeStatus::EfiUnsupported;
    c.after = c.before;
    c.after.sequence = 30;
    assert_eq!(assess(&c), Ok(Assessment::AttemptNotConfirmed));
}

#[test]
fn firmware_device_error_with_mode_change_is_unexplained() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().status = SetModeStatus::EfiDeviceError;
    assert_eq!(assess(&c), Ok(Assessment::ContradictoryReadback));
}

#[test]
fn post_exit_gop_readback_is_not_accepted_as_valid_efi_service_proof() {
    let mut c = captured_mode_change();
    c.after.stage = Stage::AfterExitBootServices;
    assert_eq!(assess(&c), Err(Error::WrongStage));
}

#[test]
fn wrong_stage_on_setmode_call_is_refused() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().stage = Stage::Unknown;
    assert_eq!(assess(&c), Err(Error::WrongStage));
}

#[test]
fn mixing_two_gop_handles_is_refused() {
    let mut c = captured_mode_change();
    c.after.device_key = 22;
    assert_eq!(assess(&c), Err(Error::DifferentDevice));
}

#[test]
fn unbound_device_key_is_refused() {
    let mut c = captured_mode_change();
    c.before.device_key = 0;
    assert_eq!(assess(&c), Err(Error::InvalidDeviceKey));
}

#[test]
fn call_device_different_from_snapshots_is_refused() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().device_key = 8;
    assert_eq!(assess(&c), Err(Error::DifferentDevice));
}

#[test]
fn out_of_order_snapshot_sequence_is_refused() {
    let mut c = captured_mode_change();
    c.after.sequence = c.before.sequence;
    assert_eq!(assess(&c), Err(Error::InvalidSequence));
}

#[test]
fn stale_or_equal_call_sequence_is_refused() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().sequence = c.before.sequence;
    assert_eq!(assess(&c), Err(Error::InvalidSequence));
}

#[test]
fn call_after_post_readback_is_refused() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().sequence = c.after.sequence + 1;
    assert_eq!(assess(&c), Err(Error::InvalidSequence));
}

#[test]
fn requested_gop_mode_outside_maxmode_is_refused() {
    let mut c = captured_mode_change();
    c.set_mode.as_mut().unwrap().requested_mode = c.before.max_modes;
    assert_eq!(assess(&c), Err(Error::RequestOutOfRange));
}

#[test]
fn invalid_post_mode_index_is_refused() {
    let mut c = captured_mode_change();
    c.after.current_mode = c.after.max_modes;
    assert_eq!(assess(&c), Err(Error::ModeOutOfRange));
}

#[test]
fn padded_pixels_per_scan_line_is_not_equal_to_width() {
    let c = captured_mode_change();
    assert!(c.after.pixels_per_scan_line > c.after.width);
    assert_eq!(assess(&c), Ok(Assessment::ConsistentWithOsModeChange));
}

#[test]
fn too_short_pixels_per_scanline_is_refused() {
    let mut c = captured_mode_change();
    c.after.pixels_per_scan_line = c.after.width - 1;
    assert_eq!(assess(&c), Err(Error::InvalidGeometry));
}

#[test]
fn unsupported_pixel_format_is_refused() {
    let mut c = captured_mode_change();
    c.after.pixel_format = 4;
    assert_eq!(assess(&c), Err(Error::UnsupportedPixelFormat));
}

#[test]
fn pixelbltonly_mode_can_be_selected_but_does_not_authorize_raw_framebuffer_draw() {
    let mut c = captured_mode_change();
    c.after.pixel_format = 3;
    c.after.framebuffer_base = 0;
    c.after.framebuffer_size = 0;
    assert_eq!(assess(&c), Ok(Assessment::ConsistentWithOsModeChange));
}
