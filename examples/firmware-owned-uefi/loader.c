/* Disposable x86-64 OVMF experiment. All image geometry is generated from
 * pinned ELF and module inputs at build time. COM2 gates are VM test controls. */
#include <efi.h>
#include <efilib.h>
#include "image.h"

#ifndef EFI_MEMORY_RO
/* UEFI table 7.6. Some GNU-EFI releases omit this attribute. */
#define EFI_MEMORY_RO 0x0000000000020000ULL
#endif

#define PAGE 4096ULL
#define MAX_DESCRIPTORS 512
#define MAP_CAPACITY (MAX_DESCRIPTORS * 128)
#define LOW_LIMIT (1ULL << 30)
struct __attribute__((packed)) mb_range { UINT32 size; UINT64 base, length; UINT32 type; };
struct module { UINT32 start, end, name, pad; };
struct boot_data {
    UINT32 info[32];
    struct module modules[3];
    char names[3][16];
    char command[96];
    struct mb_range map[MAX_DESCRIPTORS];
};
static UINT64 raw_map[MAP_CAPACITY / sizeof(UINT64)];
extern void transfer(UINT64 info, UINT64 entry, UINT64 stack) __attribute__((sysv_abi,noreturn));
extern char transfer_end[];
extern const UINT8 kernel_bytes[], surf_bytes[], picture_bytes[], audio_bytes[];

static void byte(char c) { __asm__ volatile("outb %0,%1" : : "a"(c), "Nd"((UINT16)0xe9)); }
static void word(const char *s) { while (*s) byte(*s++); }
static void hex(UINT64 x) {
    const char digits[] = "0123456789abcdef";
    for (int shift=60; shift>=0; shift-=4) byte(digits[(x >> shift) & 15]);
}
static void value(const char *s, UINT64 x) { word(s); byte('='); hex(x); byte('\n'); }
static void halt(void) __attribute__((noreturn));
static void halt(void) { for (;;) __asm__ volatile("cli; hlt"); }
static UINT8 input(UINT16 port) {
    UINT8 result;
    __asm__ volatile("inb %1,%0" : "=a"(result) : "Nd"(port));
    return result;
}
static UINT8 gate(void) {
    while (!(input(0x2fd) & 1)) __asm__ volatile("pause");
    return input(0x2f8);
}
static void copy_bytes(void *dest, const void *src, UINTN size) {
    UINT8 *d = dest; const UINT8 *s = src;
    while (size--) *d++ = *s++;
}
static void set_module(struct boot_data *b, unsigned i, UINTN base, UINTN size,
                       const UINT8 *bytes, const char *name, UINTN name_size) {
    b->modules[i] = (struct module){base, base+size, (UINTN)b->names[i], 0};
    copy_bytes((void *)base, bytes, size);
    copy_bytes(b->names[i], name, name_size);
}

static EFI_STATUS read_map(EFI_BOOT_SERVICES *bs, UINTN *key, UINTN *stride, UINTN *count) {
    UINTN size = sizeof(raw_map);
    UINT32 version = 0;
    EFI_STATUS s = uefi_call_wrapper(bs->GetMemoryMap, 5, &size, raw_map, key, stride, &version);
    if (EFI_ERROR(s)) return s;
    if (version != EFI_MEMORY_DESCRIPTOR_VERSION || *stride < sizeof(EFI_MEMORY_DESCRIPTOR)
        || size % *stride || size / *stride > MAX_DESCRIPTORS) return EFI_UNSUPPORTED;
    *count = size / *stride;
    return EFI_SUCCESS;
}

static int translate(UINTN stride, UINTN count, struct mb_range *out, int initial) {
    UINT64 owned = 0;
    for (UINTN i=0; i<count; ++i) {
        EFI_MEMORY_DESCRIPTOR *d = (void *)((UINT8 *)raw_map + i * stride);
        UINT64 lo = d->PhysicalStart;
        if (!d->NumberOfPages || (lo % PAGE) || d->NumberOfPages > (~lo / PAGE)) return 0;
        UINT64 length = d->NumberOfPages * PAGE, hi = lo + length;
        if (initial && d->Type == EfiACPIMemoryNVS) {
            word("nvs="); hex(lo); byte(','); hex(hi); byte('\n');
        }
        if (out) {
            /* Only immediately usable conventional RAM is available to the kernel. */
            UINT32 type = d->Type == EfiConventionalMemory ? 1
                        : d->Type == EfiACPIMemoryNVS ? 4 : 2;
            out[i] = (struct mb_range){20, lo, length, type};
        }
        if (!initial && hi > IMAGE_BASE && lo < ARENA_END) {
            if (d->Type != EfiLoaderData || (d->Attribute & EFI_MEMORY_RO)) return 0;
            UINT64 left = lo > IMAGE_BASE ? lo : IMAGE_BASE;
            UINT64 right = hi < ARENA_END ? hi : ARENA_END;
            /* Firmware maps are disjoint. Require contiguous coverage even when the
             * firmware emits adjacent descriptors in a different order. */
            for (UINTN j=0; j<i; ++j) {
                EFI_MEMORY_DESCRIPTOR *p = (void *)((UINT8 *)raw_map + j*stride);
                if (p->PhysicalStart < right && left < p->PhysicalStart+p->NumberOfPages*PAGE) return 0;
            }
            owned += right-left;
        }
    }
    return initial || owned == ARENA_END-IMAGE_BASE;
}

EFI_STATUS efi_main(EFI_HANDLE image, EFI_SYSTEM_TABLE *system) {
    InitializeLib(image, system);
    EFI_BOOT_SERVICES *bs = system->BootServices;
    UINTN key=0, stride=0, count=0;
    EFI_STATUS s;
    EFI_PHYSICAL_ADDRESS base=IMAGE_BASE;
    if ((UINTN)transfer > 0xffffffff || (UINTN)transfer_end > 0xffffffff
        || ARENA_END >= LOW_LIMIT || sizeof(struct boot_data) > INFO_SIZE) return EFI_UNSUPPORTED;
    s=read_map(bs,&key,&stride,&count);
    if (EFI_ERROR(s) || !translate(stride,count,NULL,1)) return EFI_UNSUPPORTED;
    value("arena_start",IMAGE_BASE); value("arena_end",ARENA_END);
    word("await-host\n");
    UINT8 mode=gate();
    if (mode!='G' && mode!='X' && mode!='R') return EFI_ABORTED;
    if (mode=='X') {
        EFI_PHYSICAL_ADDRESS occupied=IMAGE_BASE+2*PAGE;
        s=uefi_call_wrapper(bs->AllocatePages,4,AllocateAddress,EfiLoaderData,1,&occupied);
        value("control_page",s);
        if (EFI_ERROR(s)) return s;
    }
    s=uefi_call_wrapper(bs->AllocatePages,4,AllocateAddress,EfiLoaderData,(ARENA_END-IMAGE_BASE)/PAGE,&base);
    value("allocation",s);
    if (EFI_ERROR(s) || base!=IMAGE_BASE || mode=='X') {
        word("handoff-refused\n");
        uefi_call_wrapper(system->RuntimeServices->ResetSystem,4,EfiResetShutdown,EFI_SUCCESS,0,NULL);
        return EFI_ABORTED;
    }
    s=read_map(bs,&key,&stride,&count);
    if (EFI_ERROR(s) || !translate(stride,count,NULL,0)) return EFI_UNSUPPORTED;
    word("owned-before-write\n");
    SetMem((void *)(UINTN)base,ARENA_END-IMAGE_BASE,0);
    copy_bytes((void *)(UINTN)base,kernel_bytes,KERNEL_FILE_BYTES);
    struct boot_data *b=(void *)INFO_BASE;
    set_module(b,0,SURF_BASE,SURF_SIZE,surf_bytes,"surf.elf",9);
    set_module(b,1,PICTURE_BASE,PICTURE_SIZE,picture_bytes,"clip.ppm",9);
    set_module(b,2,AUDIO_BASE,AUDIO_SIZE,audio_bytes,"clip.wav",9);
    copy_bytes(b->command,GRANT,sizeof(GRANT));
    b->info[0]=4|8|0x40;
    b->info[4]=(UINTN)b->command;
    b->info[5]=3; b->info[6]=(UINTN)b->modules;
    b->info[12]=(UINTN)b->map;
    for (unsigned attempt=0; attempt<3; ++attempt) {
        s=read_map(bs,&key,&stride,&count);
        if (EFI_ERROR(s) || !translate(stride,count,b->map,0)) { word("final-map-refused\n"); halt(); }
        b->info[11]=count*sizeof(struct mb_range);
        value("map_descriptors",count);
        for (UINTN i=0;i<count;++i) {
            word("range=");hex(b->map[i].base);byte(',');hex(b->map[i].length);byte(',');hex(b->map[i].type);byte('\n');
        }
        /* Make the already obtained key stale once. The page remains allocated.
         * The subsequent retry must report a fresh map before the transfer. */
        if (mode == 'R' && attempt == 0) {
            EFI_PHYSICAL_ADDRESS extra = 0;
            s = uefi_call_wrapper(bs->AllocatePages, 4, AllocateAnyPages, EfiLoaderData, 1, &extra);
            value("stale_control_status", s);
            if (EFI_ERROR(s)) { word("control-refused\n"); halt(); }
            value("stale_control_page", extra);
        }
        s=uefi_call_wrapper(bs->ExitBootServices,2,image,key);
        value("exit_status",s);
        if (!EFI_ERROR(s)) { word("owned-handoff\n"); transfer((UINTN)b,KERNEL_ENTRY,ARENA_END); }
        if (s!=EFI_INVALID_PARAMETER) break;
    }
    word("exit-refused\n"); halt();
}
