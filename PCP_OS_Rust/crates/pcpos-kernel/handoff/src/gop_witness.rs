//! Structural assessment of a *reported* GOP mode-selection sequence.
//!
//! No firmware calls, no EDID parser, no GPU writes, no privilege or source
//! authentication, no timestamp trust and no runtime authority are provided.
//! The evidence producer must bind each event to one real GOP handle, capture
//! events before ExitBootServices, and preserve the returned EFI_STATUS.
//! This is a single-handoff diagnostic helper, not a proof of those premises.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Initiator {
    /// Instrumentation at the OS-controlled pre-ExitBootServices boundary.
    OsControlledPreExit,
    Firmware,
    Bootloader,
    NativeDriver,
    Unattributed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetModeStatus {
    EfiSuccess,
    EfiUnsupported,
    EfiDeviceError,
    Other,
}

/// The source that actually supplied this entry remains unverified here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    BootServicesActive,
    AfterExitBootServices,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeSnapshot {
    /// A stable, recorder-assigned identifier for one specific GOP handle
    /// (not an authority, cryptographic identity, or guaranteed device path).
    pub device_key: u64,
    /// Source-local increasing sequence. Do not rely on wall-clock ordering.
    pub sequence: u64,
    pub stage: Stage,
    /// GOP.Mode.Mode and GOP.Mode.MaxMode, read before ExitBootServices.
    pub current_mode: u32,
    pub max_modes: u32,
    pub width: u32,
    pub height: u32,
    /// GOP PixelsPerScanLine, NOT assumed to be width.
    pub pixels_per_scan_line: u32,
    /// Raw EFI_GRAPHICS_PIXEL_FORMAT integer, 0..=3.
    pub pixel_format: u32,
    /// Capture from GOP.Mode. These are informational, never mapped here.
    pub framebuffer_base: u64,
    pub framebuffer_size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetModeCall {
    pub device_key: u64,
    pub sequence: u64,
    pub initiator: Initiator,
    pub stage: Stage,
    /// ModeNumber parameter to EFI_GRAPHICS_OUTPUT_PROTOCOL.SetMode().
    pub requested_mode: u32,
    /// Exact returned EFI_STATUS interpreted by a trusted producer.
    pub status: SetModeStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capture {
    pub before: ModeSnapshot,
    pub set_mode: Option<SetModeCall>,
    pub after: ModeSnapshot,
    /// Whether the recorder claims no other SetMode calls could have been
    /// missed on this handle in this interval. It is not self-authenticating.
    pub claimed_complete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidDeviceKey,
    DifferentDevice,
    InvalidSequence,
    ModeOutOfRange,
    InvalidGeometry,
    UnsupportedPixelFormat,
    WrongStage,
    RequestOutOfRange,
}

/// All conclusions are about internally consistent *reported data* only.
/// They never establish who really ran, signed, intercepted or selected a
/// firmware mode, and none proves EDID preferred timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assessment {
    /// Incomplete capture; even an unchanged before/after is not a no-call proof.
    MissingCaptureCoverage,
    /// Complete as *claimed*, no SetMode event, same admitted mode.
    ReportedAlreadyActive,
    /// Complete as *claimed*, no SetMode event, mode changed unaccountably.
    UnattributedChange,
    /// Exact pre-exit, OS-attributed successful call with distinct mode and
    /// matched after-readback. Attribution still relies on producer honesty.
    ConsistentWithOsModeChange,
    /// OS-attributed successful call to a mode that was already active.
    OsReassertedMode,
    /// Successful recorded call and change, but not OS-attributed.
    OtherActorModeChange,
    /// SetMode request failed or cannot be shown to have changed mode.
    AttemptNotConfirmed,
    /// Claimed success and reported post-state contradict the request, or
    /// a failed request coincides with an unexplained change.
    ContradictoryReadback,
}

fn checked_snapshot(s: ModeSnapshot) -> Result<(), Error> {
    if s.device_key == 0 {
        return Err(Error::InvalidDeviceKey);
    }
    if s.max_modes == 0 || s.current_mode >= s.max_modes {
        return Err(Error::ModeOutOfRange);
    }
    if s.width == 0 || s.height == 0
        || s.width > 16384 || s.height > 16384
        || s.pixels_per_scan_line < s.width
    {
        return Err(Error::InvalidGeometry);
    }
    if s.pixel_format > 3 {
        return Err(Error::UnsupportedPixelFormat);
    }
    // EFI PixelBltOnly does not supply a usable *linear* framebuffer.
    // A mode report can still exist; the addresses are not treated as a grant.
    Ok(())
}

pub fn assess(c: &Capture) -> Result<Assessment, Error> {
    checked_snapshot(c.before)?;
    checked_snapshot(c.after)?;
    if c.before.device_key != c.after.device_key {
        return Err(Error::DifferentDevice);
    }
    if c.before.sequence >= c.after.sequence {
        return Err(Error::InvalidSequence);
    }
    if c.before.stage != Stage::BootServicesActive
        || c.after.stage != Stage::BootServicesActive
    {
        // A GOP readback after ExitBootServices would not be a legal EFI
        // boot-service readback. Native-driver mode changes are separate.
        return Err(Error::WrongStage);
    }
    if !c.claimed_complete {
        return Ok(Assessment::MissingCaptureCoverage);
    }
    let Some(call) = c.set_mode else {
        if c.before.current_mode == c.after.current_mode {
            return Ok(Assessment::ReportedAlreadyActive);
        }
        return Ok(Assessment::UnattributedChange);
    };
    if call.device_key != c.before.device_key {
        return Err(Error::DifferentDevice);
    }
    if call.sequence <= c.before.sequence || call.sequence >= c.after.sequence {
        return Err(Error::InvalidSequence);
    }
    if call.stage != Stage::BootServicesActive {
        return Err(Error::WrongStage);
    }
    if call.requested_mode >= c.before.max_modes {
        return Err(Error::RequestOutOfRange);
    }

    let changed = c.before.current_mode != c.after.current_mode;
    match call.status {
        SetModeStatus::EfiSuccess => {
            if c.after.current_mode != call.requested_mode {
                return Ok(Assessment::ContradictoryReadback);
            }
            if !changed {
                return Ok(Assessment::OsReassertedMode);
            }
            if call.initiator == Initiator::OsControlledPreExit {
                Ok(Assessment::ConsistentWithOsModeChange)
            } else {
                Ok(Assessment::OtherActorModeChange)
            }
        }
        SetModeStatus::EfiUnsupported | SetModeStatus::EfiDeviceError
        | SetModeStatus::Other => {
            if changed {
                Ok(Assessment::ContradictoryReadback)
            } else {
                Ok(Assessment::AttemptNotConfirmed)
            }
        }
    }
}
