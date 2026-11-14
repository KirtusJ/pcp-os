/* doomgeneric on PCP OS: the six functions the engine asks its platform for.

   This file is part of a separate program, not of the PCP OS kernel. It is written to be compiled and
   linked with doomgeneric, which is licensed under the GNU General Public License, version 2; this file is
   offered under the same licence so that the combined program can be distributed under it. */
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

#include "doomgeneric.h"
#include "doomkeys.h"
#include "pcpos.h"

static int display_fd = -1, keys_fd = -1;
static struct pcpos_display display;
static uint32_t *page[2];
static int drawing;          /* the page being drawn, which is not the one shown */
static int direct;           /* the engine draws straight into display memory */
static uint32_t *own_buffer; /* the engine's buffer when it does not */
static unsigned long frames;

/* Sound is mixed, and music is fed, whenever the engine draws or waits: not only once a picture, because some
   parts of the engine (the melt between screens, for one) wait and draw in loops of their own. */
void pcp_sound_pump(void);

void DG_Init(void)
{
    display_fd = open(PCPOS_DISPLAY_PATH, O_RDWR);
    if (display_fd < 0) {
        fprintf(stderr, "doom: there is no display this program may use\n");
        exit(2);
    }
    /* Ask for exactly the size the engine draws; failing that, take the mode there is and draw in the middle of it. */
    memset(&display, 0, sizeof display);
    display.width = DOOMGENERIC_RESX;
    display.height = DOOMGENERIC_RESY;
    if (ioctl(display_fd, PCPOS_DISPLAY_SET, &display) != 0 && ioctl(display_fd, PCPOS_DISPLAY_GET, &display) != 0) {
        fprintf(stderr, "doom: the display would not say what mode it is in\n");
        exit(2);
    }
    if (display.width < DOOMGENERIC_RESX || display.height < DOOMGENERIC_RESY) {
        fprintf(stderr, "doom: the display is %ux%u, smaller than the %dx%d this program draws\n", display.width, display.height, DOOMGENERIC_RESX, DOOMGENERIC_RESY);
        exit(2);
    }
    size_t bytes = (size_t)display.pitch * display.height * display.pages;
    uint8_t *memory = mmap(NULL, bytes, PROT_READ | PROT_WRITE, MAP_SHARED, display_fd, 0);
    if (memory == MAP_FAILED) {
        fprintf(stderr, "doom: the display's memory could not be mapped\n");
        exit(2);
    }
    page[0] = (uint32_t *)memory;
    page[1] = display.pages > 1 ? (uint32_t *)(memory + (size_t)display.pitch * display.height) : page[0];
    drawing = display.pages > 1 ? !display.shown : 0;
    own_buffer = DG_ScreenBuffer;
    direct = display.pages > 1 && display.width == DOOMGENERIC_RESX && display.height == DOOMGENERIC_RESY && display.pitch == DOOMGENERIC_RESX * 4;
    if (direct)
        DG_ScreenBuffer = page[drawing];
    keys_fd = open(PCPOS_KEYS_PATH, O_RDONLY | O_NONBLOCK);
    printf("doom on PCP OS: display %ux%u, %u page(s), %s; keyboard %s\n", display.width, display.height, display.pages,
           direct ? "drawn in place and shown by page" : "copied to the middle of the display", keys_fd >= 0 ? "present" : "ABSENT");
}

void DG_DrawFrame(void)
{
    frames++;
    pcp_sound_pump();
    if (direct) {
        ioctl(display_fd, PCPOS_DISPLAY_SHOW, drawing);
        drawing = !drawing;
        DG_ScreenBuffer = page[drawing];
        return;
    }
    uint32_t x0 = (display.width - DOOMGENERIC_RESX) / 2, y0 = (display.height - DOOMGENERIC_RESY) / 2;
    for (int y = 0; y < DOOMGENERIC_RESY; y++)
        memcpy((uint8_t *)page[drawing] + (size_t)(y0 + y) * display.pitch + x0 * 4, own_buffer + (size_t)y * DOOMGENERIC_RESX, DOOMGENERIC_RESX * 4);
    if (display.pages > 1) {
        ioctl(display_fd, PCPOS_DISPLAY_SHOW, drawing);
        drawing = !drawing;
    }
}

void DG_SleepMs(uint32_t ms)
{
    pcp_sound_pump();
    struct timespec t = { ms / 1000, (long)(ms % 1000) * 1000000L };
    nanosleep(&t, NULL);
}

uint32_t DG_GetTicksMs(void)
{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (uint32_t)(t.tv_sec * 1000 + t.tv_nsec / 1000000);
}

/* The first scan code set, keys of the first group, to what the engine calls them. Zero: not a key the engine knows. */
static const unsigned char first_group[128] = {
    [0x01] = KEY_ESCAPE, [0x02] = '1', [0x03] = '2', [0x04] = '3', [0x05] = '4', [0x06] = '5', [0x07] = '6', [0x08] = '7', [0x09] = '8', [0x0A] = '9', [0x0B] = '0',
    [0x0C] = KEY_MINUS, [0x0D] = KEY_EQUALS, [0x0E] = KEY_BACKSPACE, [0x0F] = KEY_TAB,
    [0x10] = 'q', [0x11] = 'w', [0x12] = 'e', [0x13] = 'r', [0x14] = 't', [0x15] = 'y', [0x16] = 'u', [0x17] = 'i', [0x18] = 'o', [0x19] = 'p', [0x1A] = '[', [0x1B] = ']',
    [0x1C] = KEY_ENTER, [0x1D] = KEY_FIRE,
    [0x1E] = 'a', [0x1F] = 's', [0x20] = 'd', [0x21] = 'f', [0x22] = 'g', [0x23] = 'h', [0x24] = 'j', [0x25] = 'k', [0x26] = 'l', [0x27] = ';', [0x28] = '\'', [0x29] = '`',
    [0x2A] = KEY_RSHIFT, [0x2B] = '\\',
    [0x2C] = 'z', [0x2D] = 'x', [0x2E] = 'c', [0x2F] = 'v', [0x30] = 'b', [0x31] = 'n', [0x32] = 'm', [0x33] = ',', [0x34] = '.', [0x35] = '/',
    [0x36] = KEY_RSHIFT, [0x38] = KEY_LALT, [0x39] = KEY_USE, [0x3A] = KEY_CAPSLOCK,
    [0x3B] = KEY_F1, [0x3C] = KEY_F2, [0x3D] = KEY_F3, [0x3E] = KEY_F4, [0x3F] = KEY_F5, [0x40] = KEY_F6, [0x41] = KEY_F7, [0x42] = KEY_F8, [0x43] = KEY_F9, [0x44] = KEY_F10,
    [0x47] = KEY_HOME, [0x48] = KEY_UPARROW, [0x49] = KEY_PGUP, [0x4B] = KEY_LEFTARROW, [0x4D] = KEY_RIGHTARROW, [0x4F] = KEY_END, [0x50] = KEY_DOWNARROW, [0x51] = KEY_PGDN,
    [0x52] = KEY_INS, [0x53] = KEY_DEL, [0x57] = KEY_F11, [0x58] = KEY_F12,
};

int DG_GetKey(int *pressed, unsigned char *key)
{
    struct pcpos_key k;
    while (keys_fd >= 0 && read(keys_fd, &k, sizeof k) == (ssize_t)sizeof k) {
        unsigned char doom = first_group[k.code & 0x7F];
        if (k.flags & PCPOS_KEY_SECOND) {
            /* The second group repeats numbers of the first for different keys. Those the engine knows: */
            switch (k.code) {
            case 0x1C: doom = KEY_ENTER; break;      /* the keypad's Enter */
            case 0x1D: doom = KEY_FIRE; break;       /* right Ctrl */
            case 0x38: doom = KEY_LALT; break;       /* right Alt */
            case 0x35: doom = '/'; break;
            case 0x2A: case 0x36: doom = 0; break;   /* a shift the keyboard invents around some keys: not a key press */
            default: break;                          /* arrows, Home, End, Page Up and Down, Insert, Delete: as in the first group */
            }
        }
        if (doom == 0)
            continue;
        *pressed = !(k.flags & PCPOS_KEY_RELEASED);
        *key = doom;
        return 1;
    }
    return 0;
}

void DG_SetWindowTitle(const char *title)
{
    (void)title;
}

int main(int argc, char **argv)
{
    doomgeneric_Create(argc, argv);
    for (;;)
        doomgeneric_Tick();
}
