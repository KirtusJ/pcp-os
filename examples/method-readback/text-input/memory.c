/* The freestanding Rust guest has no host C library. */
#include <stddef.h>

void *memcpy(void *dest, const void *src, size_t n) {
    unsigned char *d = dest;
    const unsigned char *s = src;
    for (size_t i = 0; i < n; ++i) d[i] = s[i];
    return dest;
}

void *memmove(void *dest, const void *src, size_t n) {
    unsigned char *d = dest;
    const unsigned char *s = src;
    if ((size_t)d <= (size_t)s) return memcpy(d, s, n);
    while (n) { --n; d[n] = s[n]; }
    return dest;
}

void *memset(void *dest, int value, size_t n) {
    unsigned char *d = dest;
    for (size_t i = 0; i < n; ++i) d[i] = (unsigned char)value;
    return dest;
}

int memcmp(const void *left, const void *right, size_t n) {
    const unsigned char *a = left, *b = right;
    for (size_t i = 0; i < n; ++i) {
        if (a[i] != b[i]) return (int)a[i] - (int)b[i];
    }
    return 0;
}

int bcmp(const void *left, const void *right, size_t n) {
    return memcmp(left, right, n);
}
