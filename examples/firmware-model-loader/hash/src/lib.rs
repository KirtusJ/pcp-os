//! Streaming SHA-256 for the freestanding EFI application, using the same algorithm crate as the worker.
#![no_std]
use core::{mem::{align_of, size_of}, panic::PanicInfo, ptr, slice};
use sha2::{Digest, Sha256};

const _: () = assert!(size_of::<Sha256>() <= 160 && align_of::<Sha256>() <= 8);

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    loop { core::hint::spin_loop(); }
}

// The C caller owns 160 aligned bytes per context, initializes it once before
// updates, and provides valid input and output buffers for the named lengths.
#[no_mangle]
pub unsafe extern "C" fn model_hash_init(ctx: *mut Sha256) {
    ptr::write(ctx, Sha256::new());
}
#[no_mangle]
pub unsafe extern "C" fn model_hash_update(ctx: *mut Sha256, buf: *const u8, len: usize) {
    (*ctx).update(slice::from_raw_parts(buf, len));
}
#[no_mangle]
pub unsafe extern "C" fn model_hash_final(ctx: *mut Sha256, out: *mut u8) {
    let digest = ptr::read(ctx).finalize();
    ptr::copy_nonoverlapping(digest.as_ptr(), out, digest.len());
}
