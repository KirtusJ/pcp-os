//! Check the negotiated display client against the old ioctl, direct drawing, and shadow presentation.
use std::cell::RefCell;
use std::panic::catch_unwind;

#[allow(dead_code)]
#[path = "../src/device.rs"]
mod device;

const PADDING: u32 = 0x505050;

#[derive(Default)]
struct Screen {
    pages: usize,
    front: usize,
    opt_in: isize,
    present_reply: isize,
    requests: Vec<usize>,
    presented: Vec<usize>,
    pixels: Vec<u32>,
    mapped: bool,
}

thread_local! {
    static SCREEN: RefCell<Screen> = RefCell::default();
}

fn setup(pages: usize, front: usize, opt_in: isize) {
    SCREEN.with_borrow_mut(|s| {
        *s = Screen {
            pages,
            front,
            opt_in,
            pixels: vec![PADDING; 10 * pages],
            ..Screen::default()
        };
    });
}

fn open(path: &[u8], _: usize) -> isize {
    match path {
        b"/dev/fb0\0" => 8,
        b"/dev/kbd\0" => 9,
        _ => panic!("unexpected device"),
    }
}

fn log(_: &[u8]) {}

fn exit(code: usize) -> ! {
    std::panic::panic_any(code)
}

fn ioctl(fd: isize, request: usize, arg: usize) -> isize {
    assert_eq!(fd, 8);
    SCREEN.with_borrow_mut(|s| {
        s.requests.push(request);
        match request {
            0x5001 | 0x5002 => {
                // Both constructors supply a writable six-word mode description.
                let info = unsafe { &mut *(arg as *mut [u32; 6]) };
                if request == 0x5002 {
                    assert_eq!(info[..2], [3, 2]);
                }
                *info = [3, 2, 20, s.pages as u32, s.front as u32, 0];
                0
            }
            0x5004 => {
                assert_eq!(arg, 0);
                assert!(!s.mapped);
                s.opt_in
            }
            0x5003 => {
                assert!(s.mapped && arg < s.pages);
                s.presented.push(arg);
                s.present_reply
            }
            _ => panic!("unexpected display ioctl {request:#x}"),
        }
    })
}

fn call(n: usize, addr: usize, len: usize, prot: usize, flags: usize, fd: usize, offset: usize) -> isize {
    assert_eq!((n, addr, prot, flags, fd, offset), (9, 0, 3, 1, 8, 0));
    SCREEN.with_borrow_mut(|s| {
        assert_eq!(s.requests.last(), Some(&0x5004));
        assert_eq!(len, s.pixels.len() * 4);
        s.mapped = true;
        s.pixels.as_mut_ptr() as isize
    })
}

#[test]
fn draw_and_show_follow_the_negotiated_contract() {
    for set_mode in [false, true] {
        for (pages, front) in [(1, 0), (2, 0), (2, 1)] {
            for reply in [-25, 0, 1] {
                setup(pages, front, reply);
                let mut display = if set_mode {
                    device::Display::new(3, 2)
                } else {
                    device::Display::current()
                };
                let dest = if pages == 1 { 0 } else { 1 - front };
                display.show(&[1, 2, 3, 4, 5, 6]);
                SCREEN.with_borrow(|s| {
                    assert_eq!(s.requests[0], if set_mode { 0x5002 } else { 0x5001 });
                    let start = 10 * dest;
                    assert_eq!(&s.pixels[start..start + 10],
                               &[1, 2, 3, PADDING, PADDING, 4, 5, 6, PADDING, PADDING]);
                });
                display.draw(|pixels, width, _, stride| pixels[stride + width - 1] = 99);
                SCREEN.with_borrow(|s| {
                    if reply == 1 || pages == 2 {
                        assert_eq!(s.presented, [dest, front]);
                    } else {
                        assert!(s.presented.is_empty());
                    }
                    assert_eq!(s.pixels[10 * front + 7], 99);
                });
            }
        }
    }
}

#[test]
fn only_enotty_selects_the_older_kernel_fallback() {
    for reply in [-1, -12, -22, 2] {
        setup(1, 0, reply);
        let reason = catch_unwind(device::Display::current).err().unwrap();
        assert_eq!(reason.downcast_ref::<usize>(), Some(&2));
        SCREEN.with_borrow(|s| assert!(!s.mapped));
    }
}

#[test]
fn a_failed_present_ends_the_guest() {
    for pages in [1, 2] {
        setup(pages, 0, 1);
        SCREEN.with_borrow_mut(|s| s.present_reply = -22);
        let reason = catch_unwind(|| {
            let mut display = device::Display::current();
            display.draw(|pixels, _, _, _| pixels[0] = 1);
        }).unwrap_err();
        assert_eq!(reason.downcast_ref::<usize>(), Some(&2));
    }
}
