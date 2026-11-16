/* User-mode file check for the PCP OS x86-64 guest ABI. */
#include <stddef.h>
#include <stdint.h>

extern const unsigned char _binary_reference_start[], _binary_reference_end[];

static long call(long number, long a, long b, long c) {
    long result;
    __asm__ volatile ("syscall" : "=a"(result)
                      : "a"(number), "D"(a), "S"(b), "d"(c)
                      : "rcx", "r11", "memory");
    return result;
}

/* Compare the opened file through EOF, including the expected-length boundary. */
int read_core(void) {
    const size_t length = (uintptr_t)_binary_reference_end - (uintptr_t)_binary_reference_start;
    static unsigned char block[512];
    size_t matched = 0;
    long fd = call(2, (long)"core-spec.md", 0, 0);
    if (fd < 0) return fd == -2 ? 2 : (fd == -13 ? 3 : 4);

    int status = 0;
    for (;;) {
        long count = call(0, fd, (long)block, sizeof block);
        if (count < 0) { status = 4; break; }
        if (count == 0) { status = matched == length ? 0 : 1; break; }
        if ((size_t)count > length - matched) { status = 1; break; }
        for (long i = 0; i < count; ++i) {
            if (block[i] != _binary_reference_start[matched + (size_t)i]) {
                status = 1;
                break;
            }
        }
        if (status) break;
        matched += (size_t)count;
    }
    if (call(3, fd, 0, 0) != 0) return 4;
    return status;
}

#ifndef READBACK_EMBEDDED
__attribute__((used, noreturn)) static void run(void) {
    static const char *const messages[] = {
        "CORE READBACK MATCH\n", "CORE READBACK DIFFERENT\n", "CORE READBACK MISSING\n",
        "CORE READBACK DENIED\n", "CORE READBACK IO ERROR\n"
    };
    int status = read_core();
    const char *line = messages[status];
    size_t length = 0;
    while (line[length]) ++length;
    call(1, 1, (long)line, (long)length);
    call(60, status, 0, 0);
    __builtin_unreachable();
}
__asm__(".global _start\n_start:\nandq $-16,%rsp\ncall run\nud2");
#endif
