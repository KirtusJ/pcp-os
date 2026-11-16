use core::alloc::{GlobalAlloc, Layout};
use pcp_game_guest::call;

/// Each live allocation owns an anonymous, writable mapping in this guest.
struct Pages;

#[global_allocator]
static ALLOCATOR: Pages = Pages;

unsafe impl GlobalAlloc for Pages {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // All allocations requested by the desk fit a page's natural alignment.
        if layout.align() > 4096 { return core::ptr::null_mut(); }
        let address = call(9, 0, layout.size(), 3, 0x22, usize::MAX, 0);
        if address < 0 { core::ptr::null_mut() } else { address as *mut u8 }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        call(11, ptr as usize, layout.size(), 0, 0, 0, 0);
    }
}
