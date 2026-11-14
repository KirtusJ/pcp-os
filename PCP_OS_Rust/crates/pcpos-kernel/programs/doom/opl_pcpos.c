/* doomgeneric on PCP OS: the OPL library's functions, for an FM synthesiser the kernel drives.

   Chocolate Doom's music player (i_oplmusic.c) plays through a small library whose usual implementations
   either emulate the chip or write its ports from a timer thread. Here there are no threads and no ports
   a program may touch. Instead, the player's timed callbacks are run a little ahead of the clock, and each
   register write they make is handed to the kernel with the time at which it is due; the kernel writes it
   to the chip from its timer, to the millisecond.

   This file is part of a separate program, not of the PCP OS kernel, and is offered under the GNU General
   Public License, version 2. OPL_InitRegisters follows opl/opl.c of Chocolate Doom (GPL-2.0). */
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "opl.h"
#include "opl_queue.h"
#include "pcpos.h"

#define AHEAD_US 80000 /* how far ahead of the clock the player's callbacks are run */

struct fm_write {
    uint32_t when_ms;
    uint16_t reg;
    uint8_t value, unused;
};

static int fm_fd = -1;
static opl_callback_queue_t *queue;
static int in_callback;
static uint64_t callback_time; /* the time of the callback being run, on the music clock */
static uint64_t paused_total, paused_since;
static int paused;
static uint64_t last_stamp;
static int index_register;
static struct fm_write pending[512];
static int npending;
static unsigned long writes, callbacks;

static uint64_t wall_us(void)
{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (uint64_t)t.tv_sec * 1000000u + t.tv_nsec / 1000;
}

/* The music clock stands still while the music is paused. */
static uint64_t music_us(void)
{
    return (paused ? paused_since : wall_us()) - paused_total;
}

static void send(void)
{
    int sent = 0;
    while (fm_fd >= 0 && sent < npending) {
        ssize_t n = write(fm_fd, pending + sent, (npending - sent) * sizeof pending[0]);
        if (n <= 0)
            break; /* the kernel has no room: these are offered again at the next pump */
        sent += n / sizeof pending[0];
    }
    if (sent < npending)
        memmove(pending, pending + sent, (npending - sent) * sizeof pending[0]);
    npending -= sent;
}

static void emit(int reg, int value)
{
    /* A write made by a callback is due at the callback's time; any other is due now. Never before one already made. */
    uint64_t t = (in_callback ? callback_time : music_us()) + paused_total;
    if (t < last_stamp)
        t = last_stamp;
    last_stamp = t;
    if (npending == (int)(sizeof pending / sizeof pending[0]))
        send();
    if (npending == (int)(sizeof pending / sizeof pending[0]))
        return; /* the kernel is not taking writes: drop this one rather than stop the game */
    pending[npending].when_ms = (uint32_t)(t / 1000);
    pending[npending].reg = (uint16_t)reg;
    pending[npending].value = (uint8_t)value;
    pending[npending].unused = 0;
    npending++;
    writes++;
}

opl_init_result_t OPL_Init(unsigned int port_base)
{
    (void)port_base;
    fm_fd = open("/dev/fm", O_WRONLY | O_NONBLOCK);
    if (fm_fd < 0) {
        printf("doom on PCP OS: no FM synthesiser this program may use; music is off\n");
        return OPL_INIT_NONE;
    }
    queue = OPL_Queue_Create();
    printf("doom on PCP OS: music played on the FM synthesiser, register writes timed by the kernel\n");
    return OPL_INIT_OPL2;
}

void OPL_Shutdown(void)
{
    if (fm_fd >= 0) {
        send();
        printf("doom on PCP OS: %lu music register writes in %lu callbacks\n", writes, callbacks);
        OPL_Queue_Destroy(queue);
        queue = NULL;
        close(fm_fd);
        fm_fd = -1;
    }
}

void OPL_SetSampleRate(unsigned int rate) { (void)rate; }

void OPL_WritePort(opl_port_t port, unsigned int value)
{
    if (port == OPL_REGISTER_PORT)
        index_register = value & 0xff;
    else if (port == OPL_REGISTER_PORT_OPL3)
        index_register = 0x100 | (value & 0xff);
    else
        emit(index_register, value);
}

/* The kernel found the chip by reading its status; a program cannot read it and does not need to. */
unsigned int OPL_ReadPort(opl_port_t port) { (void)port; return 0; }
unsigned int OPL_ReadStatus(void) { return 0; }
opl_init_result_t OPL_Detect(void) { return fm_fd >= 0 ? OPL_INIT_OPL2 : OPL_INIT_NONE; }

void OPL_WriteRegister(int reg, int value) { emit(reg, value); }

void OPL_InitRegisters(int opl3)
{
    int r;
    for (r = OPL_REGS_LEVEL; r <= OPL_REGS_LEVEL + OPL_NUM_OPERATORS; ++r)
        OPL_WriteRegister(r, 0x3f);
    /* These loops write some registers that do not exist, as Doom itself did. */
    for (r = OPL_REGS_ATTACK; r <= OPL_REGS_WAVEFORM + OPL_NUM_OPERATORS; ++r)
        OPL_WriteRegister(r, 0x00);
    for (r = 1; r < OPL_REGS_LEVEL; ++r)
        OPL_WriteRegister(r, 0x00);
    OPL_WriteRegister(OPL_REG_TIMER_CTRL, 0x60);
    OPL_WriteRegister(OPL_REG_TIMER_CTRL, 0x80);
    OPL_WriteRegister(OPL_REG_WAVEFORM_ENABLE, 0x20);
    (void)opl3; /* the chip the kernel drives is the two-operator one */
    OPL_WriteRegister(OPL_REG_FM_MODE, 0x40);
}

void OPL_SetCallback(uint64_t us, opl_callback_t callback, void *data)
{
    if (queue != NULL)
        OPL_Queue_Push(queue, callback, data, (in_callback ? callback_time : music_us()) + us);
}

void OPL_AdjustCallbacks(float factor)
{
    if (queue != NULL)
        OPL_Queue_AdjustCallbacks(queue, in_callback ? callback_time : music_us(), factor);
}

void OPL_ClearCallbacks(void)
{
    if (queue != NULL)
        OPL_Queue_Clear(queue);
}

void OPL_Lock(void) {}
void OPL_Unlock(void) {}
void OPL_Delay(uint64_t us) { (void)us; }

void OPL_SetPaused(int now_paused)
{
    if (now_paused && !paused) {
        paused_since = wall_us();
        paused = 1;
    } else if (!now_paused && paused) {
        paused = 0;
        paused_total += wall_us() - paused_since;
    }
}

/* Run every callback that falls due within the next little while, and hand what they wrote to the kernel.
   Called many times a second by the platform file. */
void pcp_music_pump(void)
{
    if (queue == NULL)
        return;
    if (!paused) {
        uint64_t horizon = music_us() + AHEAD_US;
        while (!OPL_Queue_IsEmpty(queue) && OPL_Queue_Peek(queue) <= horizon) {
            opl_callback_t callback;
            void *data;
            uint64_t t = OPL_Queue_Peek(queue);
            if (!OPL_Queue_Pop(queue, &callback, &data))
                break;
            callback_time = t;
            in_callback = 1;
            callback(data);
            in_callback = 0;
            callbacks++;
        }
    }
    send();
}
