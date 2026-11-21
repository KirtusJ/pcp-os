/* A program that asks PCP OS for what a program must not get, and for what it should, and prints one line
   for each: "ok" where the kernel answered as it must. It is run by the kernel's tests, once with every
   grant and once with none, and its output is compared with what is expected.

   Written for PCP OS; proprietary, see NOTICE.md of the kernel. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#include "../doom/pcpos.h"

static int failures;
static void check(const char *what, int ok)
{
    printf("%s %s\n", ok ? "ok  " : "FAIL", what);
    if (!ok)
        failures++;
}

static int read_note(const char *path)
{
    char bytes[4];
    int fd = open(path, O_RDONLY);
    if (fd < 0)
        return 0;
    int ok = read(fd, bytes, sizeof bytes) == sizeof bytes && memcmp(bytes, "note", sizeof bytes) == 0
             && read(fd, bytes, 1) == 0;
    close(fd);
    return ok;
}

static void check_open_failures(void)
{
    const char *path = "open-check.txt";
    const char *new_path = "new-open-check.txt";
    int fd = open(path, O_WRONLY | O_CREAT | O_EXCL, 0600);
    int created = fd >= 0 && write(fd, "note", 4) == 4;
    if (fd >= 0)
        close(fd);
    check("exclusive create writes a new file", created && read_note(path));

    fd = open(path, O_WRONLY | O_CREAT | O_EXCL | O_TRUNC, 0600);
    int existed = fd == -1 && errno == EEXIST;
    if (fd >= 0)
        close(fd);
    check("exclusive create preserves an existing file", existed && read_note(path));

    fd = open(path, O_WRONLY | O_TRUNC);
    int prepared = fd >= 0 && write(fd, "note", 4) == 4;
    if (fd >= 0)
        close(fd);

    int held[64], count = 0;
    while (count < 64) {
        fd = open("/dev/null", O_RDONLY);
        if (fd < 0)
            break;
        held[count++] = fd;
    }
    int full = fd == -1 && errno == EMFILE;
    fd = open(path, O_WRONLY | O_TRUNC);
    int truncate_refused = fd == -1 && errno == EMFILE;
    if (fd >= 0)
        close(fd);
    fd = open(new_path, O_WRONLY | O_CREAT, 0600);
    int create_refused = fd == -1 && errno == EMFILE;
    if (fd >= 0)
        close(fd);
    while (count > 0)
        close(held[--count]);

    check("failed truncation leaves the original bytes", prepared && full && truncate_refused && read_note(path));
    check("failed creation leaves the name absent",
          full && create_refused && access(new_path, F_OK) == -1 && errno == ENOENT);
    fd = open("after-close.txt", O_WRONLY | O_CREAT | O_EXCL, 0600);
    check("a released descriptor permits a new exclusive create", fd >= 0);
    if (fd >= 0)
        close(fd);
    unlink(path);
    unlink(new_path);
    unlink("after-close.txt");
}

static int same_bytes(int fd, const char *expected)
{
    char buffer[16];
    size_t n = strlen(expected);
    return lseek(fd, 0, SEEK_SET) == 0 && read(fd, buffer, sizeof buffer) == (ssize_t)n
           && memcmp(buffer, expected, n) == 0;
}

static void check_file_identity(void)
{
    const char *source = "draft.txt", *dest = "saved.txt";
    int draft = open(source, O_CREAT | O_EXCL | O_RDWR, 0600);
    int old = open(dest, O_CREAT | O_EXCL | O_RDWR | O_APPEND, 0600);
    int ready = draft >= 0 && old >= 0 && write(draft, "draft", 5) == 5 && write(old, "old", 3) == 3;
    int moved = ready && rename(source, dest) == 0;
    check("rename keeps the source descriptor attached to its bytes", moved && same_bytes(draft, "draft"));
    check("replacement keeps the previous destination open", moved && same_bytes(old, "old"));

    struct stat st;
    check("the replaced file has size three and zero names", fstat(old, &st) == 0 && st.st_size == 3 && st.st_nlink == 0);
    int saved = open(dest, O_RDWR);
    int appended = write(old, "!", 1) == 1;
    check("appending to the replaced file leaves the named file alone",
          appended && same_bytes(old, "old!") && same_bytes(saved, "draft"));

    int removed = unlink(dest) == 0 && access(dest, F_OK) == -1 && errno == ENOENT;
    int fresh = open(dest, O_CREAT | O_EXCL | O_RDWR, 0600);
    int new_bytes = fresh >= 0 && write(fresh, "new", 3) == 3;
    int edited = lseek(draft, 0, SEEK_END) == 5 && write(draft, "!", 1) == 1;
    check("open descriptors share the unlinked file after its name is reused",
          removed && new_bytes && edited && same_bytes(saved, "draft!") && same_bytes(fresh, "new"));
    check("the reused name and unlinked descriptor report their own link counts",
          fstat(saved, &st) == 0 && st.st_size == 6 && st.st_nlink == 0 &&
          stat(dest, &st) == 0 && st.st_size == 3 && st.st_nlink == 1);
    if (draft >= 0) close(draft);
    if (old >= 0) close(old);
    if (saved >= 0) close(saved);
    if (fresh >= 0) close(fresh);
    unlink(dest);
}

int main(int argc, char **argv)
{
    int granted = argc > 1 && strcmp(argv[1], "granted") == 0;
    printf("probe: run %s\n", granted ? "with every grant" : "with no grant but the console");

    /* Memory: never writable and executable at once; what is asked for is zeroed; what is given back is gone. */
    errno = 0;
    void *wx = mmap(NULL, 4096, PROT_READ | PROT_WRITE | PROT_EXEC, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    check("memory that is writable and executable at once is refused", wx == MAP_FAILED && errno == EACCES);
    unsigned char *m = mmap(NULL, 1 << 20, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    int zero = m != MAP_FAILED;
    for (int i = 0; zero && i < 1 << 20; i += 997)
        zero = m[i] == 0;
    check("a mebibyte of memory is given, and it is zeroed", zero);
    if (m != MAP_FAILED) {
        memset(m, 0xC3, 4096);
        errno = 0;
        check("making written memory both writable and executable is refused", mprotect(m, 4096, PROT_READ | PROT_WRITE | PROT_EXEC) == -1 && errno == EACCES);
        check("memory is given back", munmap(m, 1 << 20) == 0);
    }
    char *big = malloc(8 << 20);
    check("the C library's allocator gets 8 MiB", big != NULL);
    if (big) {
        memset(big, 7, 8 << 20);
        free(big);
    }

    /* Addresses that are not the program's own. */
    errno = 0;
    check("writing from the kernel's memory is refused", write(1, (void *)0x101000, 16) == -1 && errno == EFAULT);
    errno = 0;
    check("writing from memory that is not there is refused", write(1, (void *)0x9000000000, 16) == -1 && errno == EFAULT);
    errno = 0;
    check("the clock will not write into the kernel's memory", clock_gettime(CLOCK_MONOTONIC, (void *)0x101000) == -1 && errno == EFAULT);

    /* Time and floating point. */
    struct timespec a, b, nap = { 0, 50 * 1000 * 1000 };
    clock_gettime(CLOCK_MONOTONIC, &a);
    nanosleep(&nap, NULL);
    clock_gettime(CLOCK_MONOTONIC, &b);
    long ms = (b.tv_sec - a.tv_sec) * 1000 + (b.tv_nsec - a.tv_nsec) / 1000000;
    check("a sleep of 50 ms takes between 50 and 60 ms by the clock", ms >= 50 && ms <= 60);
    volatile double x = 2.0;
    check("floating point works: the square root of 2, squared, is 2", fabs(sqrt(x) * sqrt(x) - 2.0) < 1e-12);
    time_t now = time(NULL);
    check("the date is after 2025", now > 1735689600);

    /* Files. */
    errno = 0;
    check("a path that climbs out with .. names nothing", open("../doom1.wad", O_RDONLY) == -1 && errno == ENOENT);
    errno = 0;
    check("a file that is not there is not there", open("/no-such-file", O_RDONLY) == -1 && errno == ENOENT);
    int self = open("/probe.elf", O_RDONLY);
    if (granted) {
        check_open_failures();
        check_file_identity();
        unsigned char magic[4] = { 0 };
        check("a file from the loader can be read", self >= 0 && read(self, magic, 4) == 4 && magic[0] == 0x7f && magic[1] == 'E');
        errno = 0;
        check("a file from the loader cannot be opened for writing", open("/probe.elf", O_WRONLY) == -1 && errno == EACCES);
        FILE *f = fopen("note.txt", "w");
        int wrote = f != NULL && fputs("kept until power off\n", f) >= 0 && fclose(f) == 0;
        int renamed_to_self = rename("note.txt", "note.txt") == 0;
        char back[64] = "";
        f = fopen("note.txt", "r");
        int read_back = f != NULL && fgets(back, sizeof back, f) != NULL && strcmp(back, "kept until power off\n") == 0;
        if (f)
            fclose(f);
        check("a file can be written to the store and read back", wrote && read_back);
        check("renaming a stored file to itself preserves its bytes", renamed_to_self && read_back);
        errno = 0;
        check("renaming a missing file to itself reports ENOENT", rename("absent.txt", "absent.txt") == -1 && errno == ENOENT);
        check("and renamed, and removed", rename("note.txt", "note2.txt") == 0 && access("note2.txt", F_OK) == 0 && remove("note2.txt") == 0 && access("note2.txt", F_OK) == -1);
        int fb = open(PCPOS_DISPLAY_PATH, O_RDWR);
        struct pcpos_display d;
        check("the display can be opened and says what it is", fb >= 0 && ioctl(fb, PCPOS_DISPLAY_GET, &d) == 0 && d.width >= 320 && d.pages >= 1);
        check("the keyboard can be opened", open(PCPOS_KEYS_PATH, O_RDONLY | O_NONBLOCK) >= 0);
    } else {
        check("without the grant, a file from the loader cannot be opened", self == -1 && errno == EACCES);
        errno = 0;
        check("without the grant, a file cannot be written to the store", open("note.txt", O_WRONLY | O_CREAT, 0644) == -1 && errno == EACCES);
        errno = 0;
        check("without the grant, the display cannot be opened", open(PCPOS_DISPLAY_PATH, O_RDWR) == -1 && errno == EACCES);
        errno = 0;
        check("without the grant, the keyboard cannot be opened", open(PCPOS_KEYS_PATH, O_RDONLY) == -1 && errno == EACCES);
        errno = 0;
        check("without the grant, sound cannot be opened", open(PCPOS_SOUND_PATH, O_WRONLY) == -1 && (errno == EACCES || errno == ENOENT));
    }
    printf("probe: %d failure(s)\n", failures);
    if (argc > 2 && strcmp(argv[2], "fault") == 0) {
        /* Last, an act the processor refuses: the kernel ends this program and nothing else. */
        fflush(stdout);
        *(volatile int *)0x101000 = 1;
    }
    return failures ? 1 : 0;
}
