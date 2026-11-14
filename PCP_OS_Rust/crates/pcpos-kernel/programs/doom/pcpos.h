/* What a program on PCP OS needs to know that an ordinary C library does not tell it: the names of the
   devices and the three requests the display answers. Everything else is the C library's.

   Written for PCP OS; proprietary, see NOTICE.md of the kernel. This header is an interface description and
   may be included by a program under any licence. */
#ifndef PCPOS_H
#define PCPOS_H
#include <stdint.h>

#define PCPOS_DISPLAY_PATH  "/dev/fb0"   /* open, then the three requests below and mmap */
#define PCPOS_KEYS_PATH     "/dev/kbd"   /* read: two bytes for each key event */

/* The display: 32 bits to a pixel, blue in the lowest byte. `pages` pages lie one after another in its
   memory, each `pitch * height` bytes; one is shown. Where there are two, draw on the one that is not shown
   and then ask for it to be shown: nothing half-drawn is ever seen. */
struct pcpos_display {
    uint32_t width, height, pitch, pages, shown, changeable;
};
#define PCPOS_DISPLAY_GET   0x5001  /* fills the structure */
#define PCPOS_DISPLAY_SET   0x5002  /* width and height wanted, in; the whole structure, out. Fails if that mode cannot be had */
#define PCPOS_DISPLAY_SHOW  0x5003  /* the argument is the number of the page to show */

#define PCPOS_SOUND_PATH    "/dev/audio" /* write: 16-bit samples, left then right */
/* Sound: what the device plays, how many bytes it will take now, and how many are waiting to be played.
   A write takes no more than `room`; offer the rest later. What is written is heard within about 75 ms. */
struct pcpos_sound {
    uint32_t rate, channels, bits, room, waiting;
};
#define PCPOS_SOUND_GET     0x5011  /* fills the structure */

/* A key event: `code` is the key's number in the first PC scan code set (1 = Esc, 28 = Enter, ...). */
struct pcpos_key {
    uint8_t code;
    uint8_t flags;
};
#define PCPOS_KEY_RELEASED  1
#define PCPOS_KEY_SECOND    2   /* a key of the second group: the arrows, right Ctrl and Alt, Insert, Delete, ... */

#endif
