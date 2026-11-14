/* Chocolate Doom's music player is written against a newer copy of the engine's headers than doomgeneric
   carries. These are the few declarations it uses that doomgeneric's headers lack, as Chocolate Doom
   declares them (GPL-2.0). Included before i_oplmusic.c and midifile.c, and nowhere else. */
#ifndef CHOC_COMPAT_H
#define CHOC_COMPAT_H
#include <stddef.h>
#ifndef PACKED_STRUCT
#define PACKED_STRUCT(...) struct __VA_ARGS__ __attribute__((packed))
#endif
typedef enum {
    opl_doom1_1_666, /* Doom 1 v1.666 */
    opl_doom2_1_666, /* Doom 2 v1.666, Hexen, Heretic */
    opl_doom_1_9     /* Doom v1.9, Strife */
} opl_driver_ver_t;
void I_SetOPLDriverVer(opl_driver_ver_t ver);
void I_OPL_DevMessages(char *result, size_t result_len);

/* Four helpers Chocolate Doom has and doomgeneric lacks: file functions that differ from the C library's only
   on Windows, a reallocation that stops the program instead of returning nothing, and byte swapping, which
   Chocolate Doom takes from a library that is not here. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#define M_fopen fopen
#define M_remove remove
static inline void *I_Realloc(void *p, size_t n)
{
    void *q = realloc(p, n);
    if (q == NULL && n != 0) {
        fprintf(stderr, "doom: out of memory\n");
        exit(1);
    }
    return q;
}
#define SDL_SwapBE16(x) __builtin_bswap16((uint16_t)(x))
#define SDL_SwapBE32(x) __builtin_bswap32((uint32_t)(x))
#endif
