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
_Static_assert(sizeof(GRANT) <= sizeof(((struct boot_data *)0)->command), "kernel command does not fit");
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
#ifdef PCP_USB_VM
/* This unattended experiment is limited to the QEMU TCG fixture. */
static int tcg_fixture(void) {
    UINT32 a, b, c, d;
    __asm__ volatile("cpuid" : "=a"(a), "=b"(b), "=c"(c), "=d"(d) : "a"(1), "c"(0));
    if (!(c & (1U << 31))) return 0;
    __asm__ volatile("cpuid" : "=a"(a), "=b"(b), "=c"(c), "=d"(d) : "a"(0x40000000), "c"(0));
    return b == 0x54474354 && c == 0x43544743 && d == 0x47435447; /* TCGTCGTCGTCG */
}

/* Allocation failed while firmware still owns the console. Stay in this image
 * until the person confirms power-off or uses the machine's power button. */
static EFI_STATUS refusal_message(SIMPLE_TEXT_OUTPUT_INTERFACE *out, CHAR16 *action) {
    uefi_call_wrapper(out->ClearScreen, 1, out);
    EFI_STATUS s = uefi_call_wrapper(out->OutputString, 2, out,
        L"PCP OS could not start.\r\nFirmware could not reserve the memory needed by the kernel.\r\n"
        L"You can switch off the computer with its power button.\r\n");
    if (EFI_ERROR(s) || !action) return s;
    return uefi_call_wrapper(out->OutputString, 2, out, action);
}

static void allocation_refusal(EFI_SYSTEM_TABLE *system) __attribute__((noreturn));
static void allocation_refusal(EFI_SYSTEM_TABLE *system) {
    EFI_BOOT_SERVICES *bs = system->BootServices;
    SIMPLE_TEXT_OUTPUT_INTERFACE *out = system->ConOut;
    SIMPLE_INPUT_INTERFACE *in = system->ConIn;
    EFI_STATUS watchdog = uefi_call_wrapper(bs->SetWatchdogTimer, 4, 0, 0, 0, NULL);
    value("refusal_watchdog_status", watchdog);
    if (!out) halt();
    BOOLEAN confirm = FALSE;
    if (!EFI_ERROR(watchdog) && in && in->WaitForKey &&
        !EFI_ERROR(uefi_call_wrapper(in->Reset, 2, in, FALSE))) {
        BOOLEAN redraw = TRUE;
        for (;;) {
            if (redraw && EFI_ERROR(refusal_message(out, confirm
                    ? L"Switch off? Enter switches off. Esc cancels.\r\n"
                    : L"Press F10 to request power off.\r\n"))) break;
            redraw = FALSE;
            UINTN index;
            EFI_INPUT_KEY key;
            if (EFI_ERROR(uefi_call_wrapper(bs->WaitForEvent, 3, 1, &in->WaitForKey, &index))) break;
            EFI_STATUS s = uefi_call_wrapper(in->ReadKeyStroke, 2, in, &key);
            if (s == EFI_NOT_READY) continue;
            if (EFI_ERROR(s)) break;
            if ((!confirm && key.ScanCode == SCAN_F10) || (confirm && key.ScanCode == SCAN_ESC)) {
                confirm = !confirm;
                redraw = TRUE;
            } else if (confirm && key.ScanCode == 0 && key.UnicodeChar == CHAR_CARRIAGE_RETURN) {
                word("refusal-power-confirmed\n");
                uefi_call_wrapper(system->RuntimeServices->ResetSystem, 4, EfiResetShutdown, EFI_SUCCESS, 0, NULL);
                break;
            }
        }
    }
    refusal_message(out, NULL);
    halt();
}

static int matches(UINTN start, const UINT8 *expected, UINTN length) {
    const volatile UINT8 *actual = (const volatile UINT8 *)start;
    for (UINTN i = 0; i < length; ++i)
        if (actual[i] != (expected ? expected[i] : 0)) return 0;
    return 1;
}
#else
static UINT8 input(UINT16 port) {
    UINT8 result;
    __asm__ volatile("inb %1,%0" : "=a"(result) : "Nd"(port));
    return result;
}
static UINT8 gate(void) {
    while (!(input(0x2fd) & 1)) __asm__ volatile("pause");
    return input(0x2f8);
}
#endif
static void copy_bytes(void *dest, const void *src, UINTN size) {
    UINT8 *d = dest; const UINT8 *s = src;
    while (size--) *d++ = *s++;
}
#ifdef PCP_USB_VM
/* Multiboot 1 puts the direct-colour descriptor at byte 88 of the boot information. */
struct __attribute__((packed)) mb_framebuffer {
    UINT64 address;
    UINT32 pitch, width, height;
    UINT8 bits, type, red_pos, red_bits, green_pos, green_bits, blue_pos, blue_bits;
};
_Static_assert(sizeof(struct mb_framebuffer) == 28, "Multiboot framebuffer layout");
_Static_assert(88 + sizeof(struct mb_framebuffer) <= sizeof(((struct boot_data *)0)->info),
               "Multiboot framebuffer must fit the boot header");

static void describe_console(EFI_SYSTEM_TABLE *system, struct boot_data *boot) {
    EFI_GUID guid = EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID;
    EFI_GRAPHICS_OUTPUT_PROTOCOL *gop = NULL;
    EFI_STATUS status = uefi_call_wrapper(system->BootServices->HandleProtocol, 3,
                                          system->ConsoleOutHandle, &guid, (void **)&gop);
    if (EFI_ERROR(status) || !gop || !gop->Mode) return;
    EFI_GRAPHICS_OUTPUT_PROTOCOL_MODE *mode = gop->Mode;
    EFI_GRAPHICS_OUTPUT_MODE_INFORMATION *info = mode->Info;
    if (!info || mode->SizeOfInfo < sizeof(*info)
        || info->PixelFormat != PixelBlueGreenRedReserved8BitPerColor
        || !info->HorizontalResolution || !info->VerticalResolution
        || info->PixelsPerScanLine < info->HorizontalResolution
        || info->PixelsPerScanLine > 0xffffffffU / 4) return;
    UINT32 pitch = info->PixelsPerScanLine * 4;
    UINT64 bytes = (UINT64)pitch * info->VerticalResolution;
    if (!mode->FrameBufferBase || (mode->FrameBufferBase & 3)
        || bytes > mode->FrameBufferSize || mode->FrameBufferBase > ~((UINT64)0) - bytes) return;
    struct mb_framebuffer fb = {
        mode->FrameBufferBase, pitch, info->HorizontalResolution, info->VerticalResolution,
        32, 1, 16, 8, 8, 8, 0, 8
    };
    copy_bytes((UINT8 *)boot->info + 88, &fb, sizeof(fb));
    boot->info[0] |= 1U << 12;
    value("console_framebuffer", fb.address);
    value("console_pitch", fb.pitch);
}
#endif

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
#ifdef PCP_USB_VM
    if (!tcg_fixture()) return EFI_UNSUPPORTED;
#endif
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
#ifdef PCP_USB_VM
    const UINT8 mode = 'G';
    word("removable-vm\n");
#else
    word("await-host\n");
    UINT8 mode=gate();
    if (mode!='G' && mode!='X' && mode!='R') return EFI_ABORTED;
    if (mode=='X') {
        EFI_PHYSICAL_ADDRESS occupied=IMAGE_BASE+2*PAGE;
        s=uefi_call_wrapper(bs->AllocatePages,4,AllocateAddress,EfiLoaderData,1,&occupied);
        value("control_page",s);
        if (EFI_ERROR(s)) return s;
    }
#endif
    s=uefi_call_wrapper(bs->AllocatePages,4,AllocateAddress,EfiLoaderData,(ARENA_END-IMAGE_BASE)/PAGE,&base);
    value("allocation",s);
    if (EFI_ERROR(s) || base!=IMAGE_BASE || mode=='X') {
        word("handoff-refused\n");
#ifdef PCP_USB_VM
        allocation_refusal(system);
#else
        uefi_call_wrapper(system->RuntimeServices->ResetSystem,4,EfiResetShutdown,EFI_SUCCESS,0,NULL);
        return EFI_ABORTED;
#endif
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
#ifdef PCP_USB_VM
    /* The kernel and modules come from this measured build. Read the destination
     * back before handoff, including every zero-filled kernel tail. */
    if (!matches(IMAGE_BASE, kernel_bytes, KERNEL_FILE_BYTES)
        || !matches(IMAGE_BASE + KERNEL_FILE_BYTES, NULL, SURF_BASE - IMAGE_BASE - KERNEL_FILE_BYTES)
        || !matches(SURF_BASE, surf_bytes, SURF_SIZE)
        || !matches(PICTURE_BASE, picture_bytes, PICTURE_SIZE)
        || !matches(AUDIO_BASE, audio_bytes, AUDIO_SIZE)) {
        word("content-refused\n"); halt();
    }
    word("content-verified\n");
    describe_console(system, b);
#else
    word("await-content-check\n");
    if (gate() != 'C') { word("content-refused\n"); halt(); }
#endif
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
