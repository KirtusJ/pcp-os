/* Test-only Linux interposition. Suspend one target file open before or after the
   kernel sees it; all register opens and every other process run as usual. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

static void gate(const char *path, const char *stage) {
    const char *target = getenv("PROBE_TARGET"), *when = getenv("PROBE_STAGE");
    if (!target || !when || strcmp(target, path) || strcmp(when, stage)) return;
    int marker = syscall(SYS_openat, AT_FDCWD, getenv("PROBE_ENTERED"), O_CREAT|O_WRONLY, 0600);
    if (marker < 0) _exit(98);
    close(marker);
    for (int i = 0; i < 10000; ++i) {
        if (access(getenv("PROBE_RELEASE"), F_OK) == 0) return;
        usleep(1000);
    }
    _exit(99);
}

static int opened(int dir, const char *path, int flags, mode_t mode) {
    gate(path, "before");
    int fd = syscall(SYS_openat, dir, path, flags, mode);
    if (fd >= 0) gate(path, "after");
    return fd;
}

#define MODE_ARG() \
    mode_t mode = 0; \
    if ((flags & O_CREAT) || (flags & O_TMPFILE) == O_TMPFILE) { \
        va_list ap; va_start(ap, flags); mode = va_arg(ap, mode_t); va_end(ap); \
    }
int open(const char *path, int flags, ...) { MODE_ARG(); return opened(AT_FDCWD, path, flags, mode); }
int open64(const char *path, int flags, ...) { MODE_ARG(); return opened(AT_FDCWD, path, flags, mode); }
int openat(int dir, const char *path, int flags, ...) { MODE_ARG(); return opened(dir, path, flags, mode); }
int openat64(int dir, const char *path, int flags, ...) { MODE_ARG(); return opened(dir, path, flags, mode); }
