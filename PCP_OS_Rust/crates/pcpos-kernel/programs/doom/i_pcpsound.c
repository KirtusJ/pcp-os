/* doomgeneric on PCP OS: sound effects. The engine's sixteen channels are mixed here, in the program, into
   the one stream the sound device plays, and written a little ahead of where the device is playing.

   This file is part of a separate program, not of the PCP OS kernel, and is offered under the GNU General
   Public License, version 2, as doomgeneric is. The layout of a sound lump and the way volume and
   separation become left and right levels follow Chocolate Doom's i_sdlsound.c (GPL-2.0). */
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#include "deh_str.h"
#include "i_sound.h"
#include "m_misc.h"
#include "w_wad.h"
#include "z_zone.h"
#include "pcpos.h"

/* Two settings the engine's configuration names, which belong to a sound library that is not used here. */
int use_libsamplerate = 0;
float libsamplerate_scale = 0.65f;

#define CHANNELS 16
#define MOST_PAIRS 2048 /* mixed in one update: 43 ms */

struct voice {
    const unsigned char *data; /* unsigned 8-bit samples, or NULL when the channel is silent */
    unsigned length;
    unsigned position; /* in samples, 16.16 */
    unsigned step;     /* samples of the lump for each pair played, 16.16 */
    int left, right;   /* 0..255 */
};

static struct voice voices[CHANNELS];
static int sound_fd = -1;
static unsigned out_rate = 48000;
static boolean use_prefix;
static short mixed[MOST_PAIRS * 2];
static unsigned long pairs_written, updates, short_updates;

static boolean pcp_init(boolean use_sfx_prefix)
{
    struct pcpos_sound s;
    use_prefix = use_sfx_prefix;
    sound_fd = open(PCPOS_SOUND_PATH, O_WRONLY | O_NONBLOCK);
    if (sound_fd < 0 || ioctl(sound_fd, PCPOS_SOUND_GET, &s) != 0 || s.channels != 2 || s.bits != 16 || s.rate == 0) {
        printf("doom on PCP OS: no sound device this program may use; sound effects are off\n");
        sound_fd = -1;
        return false;
    }
    out_rate = s.rate;
    printf("doom on PCP OS: sound effects mixed here and played at %u Hz\n", out_rate);
    return true;
}

static void pcp_shutdown(void)
{
    if (sound_fd >= 0)
        printf("doom on PCP OS: %lu pairs of samples written in %lu updates\n", pairs_written, updates);
}

static int pcp_lump(sfxinfo_t *sfx)
{
    char name[16];
    if (sfx->link != NULL)
        sfx = sfx->link;
    if (use_prefix)
        M_snprintf(name, sizeof name, "ds%s", DEH_String(sfx->name));
    else
        M_StringCopy(name, DEH_String(sfx->name), sizeof name);
    return W_GetNumForName(name);
}

static void levels(struct voice *v, int vol, int sep)
{
    int left = ((254 - sep) * vol) / 127, right = (sep * vol) / 127;
    v->left = left < 0 ? 0 : left > 255 ? 255 : left;
    v->right = right < 0 ? 0 : right > 255 ? 255 : right;
}

static void pcp_params(int channel, int vol, int sep)
{
    if (channel >= 0 && channel < CHANNELS)
        levels(&voices[channel], vol, sep);
}

static int pcp_start(sfxinfo_t *sfx, int channel, int vol, int sep)
{
    if (sound_fd < 0 || channel < 0 || channel >= CHANNELS)
        return -1;
    int lump = sfx->lumpnum;
    unsigned len = W_LumpLength(lump);
    const unsigned char *d = W_CacheLumpNum(lump, PU_STATIC); /* kept: a sound is small and is played again */
    struct voice *v = &voices[channel];
    v->data = NULL;
    /* A sound lump: format 3, the sample rate, the number of samples, then the samples with sixteen bytes of
       padding before and after, which are not played. */
    if (len < 8 || d[0] != 0x03 || d[1] != 0x00)
        return -1;
    unsigned rate = d[2] | d[3] << 8;
    unsigned count = d[4] | d[5] << 8 | d[6] << 16 | (unsigned)d[7] << 24;
    if (count > len - 8 || count <= 48 || rate == 0)
        return -1;
    v->length = count - 32;
    v->position = 0;
    v->step = (unsigned)(((unsigned long long)rate << 16) / out_rate);
    levels(v, vol, sep);
    v->data = d + 8 + 16;
    return channel;
}

static void pcp_stop(int channel)
{
    if (channel >= 0 && channel < CHANNELS)
        voices[channel].data = NULL;
}

static boolean pcp_playing(int channel)
{
    return channel >= 0 && channel < CHANNELS && voices[channel].data != NULL;
}

void pcp_music_pump(void);

/* Called for every picture the engine draws, and whenever the engine waits: mix as much as the device will take now. */
void pcp_sound_pump(void);
static void pcp_update(void)
{
    pcp_sound_pump();
}

void pcp_sound_pump(void)
{
#ifdef PCP_OPL_MUSIC
    pcp_music_pump();
#endif
    struct pcpos_sound s;
    if (sound_fd < 0 || ioctl(sound_fd, PCPOS_SOUND_GET, &s) != 0)
        return;
    updates++;
    unsigned pairs = s.room / 4;
    if (pairs > MOST_PAIRS)
        pairs = MOST_PAIRS;
    if (pairs == 0)
        return;
    if (s.waiting == 0 && pairs_written != 0)
        short_updates++; /* the device had nothing left: a gap was heard */
    static int sum[MOST_PAIRS * 2];
    memset(sum, 0, pairs * 2 * sizeof sum[0]);
    for (int c = 0; c < CHANNELS; c++) {
        struct voice *v = &voices[c];
        if (v->data == NULL)
            continue;
        for (unsigned n = 0; n < pairs; n++) {
            unsigned i = v->position >> 16;
            if (i >= v->length) {
                v->data = NULL;
                break;
            }
            /* Between two samples of the lump, the value on the line between them. */
            int a = v->data[i] - 128, b = (i + 1 < v->length ? v->data[i + 1] : v->data[i]) - 128;
            int frac = (v->position >> 8) & 0xff;
            int sample = (a * (256 - frac) + b * frac); /* 8-bit sample scaled by 256: 16 bits */
            sum[n * 2] += sample * v->left / 256;
            sum[n * 2 + 1] += sample * v->right / 256;
            v->position += v->step;
        }
    }
    for (unsigned n = 0; n < pairs * 2; n++) {
        int x = sum[n];
        mixed[n] = x > 32767 ? 32767 : x < -32768 ? -32768 : x;
    }
    ssize_t done = write(sound_fd, mixed, pairs * 4);
    if (done > 0)
        pairs_written += done / 4;
}

static void pcp_cache(sfxinfo_t *sounds, int num_sounds)
{
    (void)sounds;
    (void)num_sounds;
}

static snddevice_t sound_devices[] = { SNDDEVICE_SB, SNDDEVICE_PAS, SNDDEVICE_GUS, SNDDEVICE_WAVEBLASTER, SNDDEVICE_SOUNDCANVAS, SNDDEVICE_AWE32 };

sound_module_t DG_sound_module = {
    sound_devices, sizeof sound_devices / sizeof sound_devices[0],
    pcp_init, pcp_shutdown, pcp_lump, pcp_update, pcp_params, pcp_start, pcp_stop, pcp_playing, pcp_cache,
};

/* Music is another module: Chocolate Doom's OPL player (i_oplmusic.c), built under the name the engine
   looks for, playing through opl_pcpos.c. Without it, a module that says there is no music: */
#ifndef PCP_OPL_MUSIC
static boolean no_music_init(void) { return false; }
static void no_music_void(void) {}
static void no_music_volume(int v) { (void)v; }
static void *no_music_register(void *data, int len) { (void)data; (void)len; return NULL; }
static void no_music_unregister(void *h) { (void)h; }
static void no_music_play(void *h, boolean looping) { (void)h; (void)looping; }
static boolean no_music_playing(void) { return false; }
static snddevice_t music_devices[] = { SNDDEVICE_ADLIB, SNDDEVICE_SB, SNDDEVICE_PAS, SNDDEVICE_GUS, SNDDEVICE_WAVEBLASTER, SNDDEVICE_SOUNDCANVAS, SNDDEVICE_GENMIDI, SNDDEVICE_AWE32 };

music_module_t DG_music_module = {
    music_devices, sizeof music_devices / sizeof music_devices[0],
    no_music_init, no_music_void, no_music_volume, no_music_void, no_music_void, no_music_register, no_music_unregister, no_music_play, no_music_void, no_music_playing, no_music_void,
};
#endif
