/* x86-64 UEFI, PCP OS Multiboot 1, model-memory handoff. No writes to media. */
#include <efi.h>
#include <efilib.h>

struct segment { UINT32 address, offset, size; };
#include "config.h"

#define PAGE_BYTES 4096ULL
#define MIB (1ULL << 20)
#define LOW_LIMIT (1ULL << 30)
#define HIGH_START (4ULL << 30)
#define HIGH_END (68ULL << 30)
#define MAX_PIECES 8
#define MODEL_RAM 0x80000001U
#define MAP_ENTRIES ((64 * 1024) / 24)
#define IO_BLOCK (1U << 20)
#define ARRAY_LEN(a) (sizeof(a) / sizeof((a)[0]))

typedef struct { UINT64 words[20]; } hash_state;
extern void model_hash_init(hash_state *);
extern void model_hash_update(hash_state *, const UINT8 *, UINTN);
extern void model_hash_final(hash_state *, UINT8 *);
extern void transfer(UINT64, UINT64, UINT64) __attribute__((noreturn));
extern char transfer_end[];

struct piece { UINT64 start, size; }; /* page-aligned allocation, in logical shard order */
struct mb_module { UINT32 start, end, name, reserved; };
struct __attribute__((packed)) mb_range { UINT32 size; UINT64 base, length; UINT32 type; };
struct boot_data {
    UINT32 info[32];
    struct mb_module mods[3];
    char names[3][20];
    char command[1024];
    struct mb_range ranges[MAP_ENTRIES];
    UINT8 stack[16384] __attribute__((aligned(16)));
};

static EFI_BOOT_SERVICES *bs;
static EFI_SYSTEM_TABLE *st;
static const CHAR16 *reason;
static EFI_FILE_HANDLE root, folder, opened;
static struct piece owned[16], pieces[MAX_PIECES];
static UINTN owned_count, piece_count;
static struct boot_data *boot;
static UINT8 *inputs[4];
static const UINTN sizes[] = { INPUT_0_SIZE, INPUT_1_SIZE, INPUT_2_SIZE, INPUT_3_SIZE };
static CHAR16 *const names[] = { L"KERNEL.ELF", L"TAILOR.ELF", L"WORKER.ELF", L"model.manifest" };
static UINT8 raw_map[256 * 1024] __attribute__((aligned(8)));
static UINTN stride, map_count, map_key;
static UINT8 seen_shards[(SHARD_COUNT + 7) / 8];
static UINTN range_count;
static BOOLEAN exit_attempted;

static UINT64 minimum(UINT64 a, UINT64 b) { return a < b ? a : b; }
static UINT64 maximum(UINT64 a, UINT64 b) { return a > b ? a : b; }
static UINT64 page_size(UINT64 n) { return (n + PAGE_BYTES - 1) & ~(PAGE_BYTES - 1); }
static BOOLEAN reject(const CHAR16 *message) { reason = message; return FALSE; }
static BOOLEAN same(const UINT8 *a, const UINT8 *b, UINTN size) {
    for (UINTN i = 0; i < size; i++) if (a[i] != b[i]) return FALSE;
    return TRUE;
}
static void say(const CHAR16 *text) {
    if (st->ConOut) st->ConOut->OutputString(st->ConOut, (CHAR16 *)text);
}
static void number(UINT64 n) {
    CHAR16 digits[24], *pos = digits + ARRAY_LEN(digits) - 1;
    *pos = 0;
    do { *--pos = L'0' + n % 10; n /= 10; } while (n);
    say(pos);
}
static EFI_MEMORY_DESCRIPTOR *descriptor(UINTN i) {
    return (EFI_MEMORY_DESCRIPTOR *)(raw_map + i * stride);
}
static BOOLEAN read_map(void) {
    UINTN bytes = sizeof(raw_map);
    UINT32 version = 0;
    EFI_STATUS s = bs->GetMemoryMap(&bytes, (EFI_MEMORY_DESCRIPTOR *)raw_map, &map_key, &stride, &version);
    if (EFI_ERROR(s) || version != EFI_MEMORY_DESCRIPTOR_VERSION || stride < sizeof(EFI_MEMORY_DESCRIPTOR)
            || (stride % 8) != 0 || bytes % stride || !bytes)
        return reject(L"The firmware memory map cannot be read in the supported format.");
    map_count = bytes / stride;
    for (UINTN i = 0; i < map_count; i++) {
        EFI_MEMORY_DESCRIPTOR *d = descriptor(i);
        if (d->PhysicalStart % PAGE_BYTES || !d->NumberOfPages || d->NumberOfPages > (~d->PhysicalStart)/PAGE_BYTES)
            return reject(L"The firmware memory map contains an invalid range.");
    }
    return TRUE;
}
static UINT64 end_of(const EFI_MEMORY_DESCRIPTOR *d) { return d->PhysicalStart + d->NumberOfPages * PAGE_BYTES; }
/* The kernel treats ordinary RAM as interchangeable for code, data and page tables.
 * Reserve descriptors advertising special purpose, persistence or restricted access. */
static BOOLEAN ordinary_ram(const EFI_MEMORY_DESCRIPTOR *d) {
    const UINT64 excluded = EFI_MEMORY_RUNTIME | EFI_MEMORY_SP | EFI_MEMORY_NV |
        EFI_MEMORY_RP | EFI_MEMORY_WP | EFI_MEMORY_RO | EFI_MEMORY_XP;
    return (d->Attribute & EFI_MEMORY_WB) && !(d->Attribute & excluded);
}
static BOOLEAN conventional(const EFI_MEMORY_DESCRIPTOR *d) {
    return d->Type == EfiConventionalMemory && ordinary_ram(d);
}
static BOOLEAN claim(UINT64 start, UINT64 bytes) {
    EFI_PHYSICAL_ADDRESS address = start;
    if (!bytes || bytes % PAGE_BYTES || owned_count == ARRAY_LEN(owned))
        return reject(L"The requested memory layout cannot be represented.");
    if (EFI_ERROR(bs->AllocatePages(AllocateAddress, EfiLoaderData, bytes/PAGE_BYTES, &address)))
        return reject(L"Firmware could not reserve the requested memory. No model was started.");
    owned[owned_count++] = (struct piece){address, bytes};
    if (address != start) return reject(L"Firmware returned a different memory address.");
    return TRUE;
}

/* Choose the largest eight conventional ranges in the kernel's high window.
 * The top eight give the maximum representable total; physical order is
 * immaterial because the command line records the order used for the copy. */
static BOOLEAN reserve_high(void) {
    struct piece best[MAX_PIECES] = {{0, 0}};
    if (!read_map()) return FALSE;
    for (UINTN i = 0; i < map_count; i++) {
        EFI_MEMORY_DESCRIPTOR *d = descriptor(i);
        if (!conventional(d)) continue;
        UINT64 start = maximum(HIGH_START, d->PhysicalStart), end = minimum(HIGH_END, end_of(d));
        if (end <= start) continue;
        struct piece item = {start, end-start};
        for (UINTN j = 0; j < MAX_PIECES; j++) {
            if (item.size > best[j].size || (item.size == best[j].size && item.start < best[j].start)) {
                struct piece old = best[j]; best[j] = item; item = old;
            }
        }
    }
    UINT64 available = 0, needed = page_size(MODEL_BYTES);
    for (UINTN i = 0; i < MAX_PIECES; i++) available += best[i].size;
    if (available < needed) {
        say(L"Model memory required: "); number(needed);
        say(L" bytes. Available in at most eight high ranges: "); number(available); say(L" bytes.\r\n");
        return reject(L"There is not enough usable high memory for this model.");
    }
    for (UINTN i = 0; needed; i++) {
        UINT64 take = minimum(needed, best[i].size);
        if (!claim(best[i].start, take)) return FALSE;
        pieces[piece_count++] = (struct piece){best[i].start, take};
        needed -= take;
    }
    return TRUE;
}

/* Group low payloads near the kernel. Choosing the top of the first GiB could
 * leave no low allocator pool beyond the last loaded module. */
static BOOLEAN reserve_low(UINT64 after, UINT64 bytes, UINT8 **ptr) {
    if (!read_map()) return FALSE;
    UINT64 base = LOW_LIMIT;
    bytes = page_size(bytes);
    for (UINTN i = 0; i < map_count; i++) {
        EFI_MEMORY_DESCRIPTOR *d = descriptor(i);
        if (!conventional(d)) continue;
        UINT64 a = maximum(after, d->PhysicalStart), b = minimum(LOW_LIMIT, end_of(d));
        if (a < b && bytes <= b-a && a < base) base = a;
    }
    if (base == LOW_LIMIT) return reject(L"Not enough usable low memory for the kernel and its programs.");
    if (!claim(base, bytes)) return FALSE;
    *ptr = (UINT8 *)(UINTN)base;
    bs->SetMem(*ptr, bytes, 0);
    return TRUE;
}
static BOOLEAN reserve_all(void) {
    if ((UINTN)transfer >= (1ULL<<32) || (UINTN)transfer_end > (1ULL<<32))
        return reject(L"Firmware placed the execution transition above its supported 32-bit address range.");
    if (!reserve_high()) return FALSE;
    if (!read_map()) return FALSE;
    BOOLEAN free = FALSE;
    for (UINTN i = 0; i < map_count; i++) {
        EFI_MEMORY_DESCRIPTOR *d = descriptor(i);
        if (conventional(d) && d->PhysicalStart <= KERNEL_BASE && end_of(d) >= KERNEL_END) free = TRUE;
    }
    if (!free) return reject(L"The kernel's linked physical range is unavailable on this machine.");
    if (!claim(KERNEL_BASE, KERNEL_END-KERNEL_BASE)) return FALSE;
    bs->SetMem((void *)(UINTN)KERNEL_BASE, KERNEL_END-KERNEL_BASE, 0);
    UINT8 *info;
    if (!reserve_low(KERNEL_END, sizeof(*boot), &info)) return FALSE;
    boot = (struct boot_data *)info;
    for (UINTN i = 0; i < ARRAY_LEN(sizes); i++)
        if (!reserve_low(KERNEL_END, sizes[i], &inputs[i])) return FALSE;
    return TRUE;
}

static BOOLEAN close_handle(EFI_FILE_HANDLE *handle) {
    if (!*handle) return TRUE;
    EFI_FILE_HANDLE file = *handle;
    *handle = NULL;
    return !EFI_ERROR(file->Close(file));
}
static BOOLEAN open_volume(EFI_HANDLE image) {
    EFI_LOADED_IMAGE *app = NULL;
    EFI_FILE_IO_INTERFACE *volume = NULL;
    if (EFI_ERROR(bs->HandleProtocol(image,&LoadedImageProtocol,(void **)&app)) || !app
            || EFI_ERROR(bs->HandleProtocol(app->DeviceHandle,&FileSystemProtocol,(void **)&volume)) || !volume
            || EFI_ERROR(volume->OpenVolume(volume,&root)) || !root)
        return reject(L"The loader could not open its own boot volume.");
    if (EFI_ERROR(root->Open(root,&folder,L"PCPOS",EFI_FILE_MODE_READ,0)) || !folder)
        return reject(L"The PCPOS folder is missing or unreadable on the boot volume.");
    return TRUE;
}
static BOOLEAN open_input(CHAR16 *name) {
    if (EFI_ERROR(folder->Open(folder,&opened,name,EFI_FILE_MODE_READ,0)) || !opened) {
        say(L"File: "); say(name); say(L"\r\n");
        return reject(L"A required file is missing or unreadable.");
    }
    return TRUE;
}
static BOOLEAN read_bytes(UINT8 *buffer, UINTN size) {
    while (size) {
        UINTN actual = size;
        if (EFI_ERROR(opened->Read(opened,&actual,buffer)) || !actual || actual > size)
            return reject(L"A required file could not be read to its expected length.");
        buffer += actual; size -= actual;
    }
    return TRUE;
}
static BOOLEAN finish_file(void) {
    UINT8 extra; UINTN count = 1;
    if (EFI_ERROR(opened->Read(opened,&count,&extra)) || count != 0)
        return reject(L"A required file extends past its expected length or could not be checked.");
    if (!close_handle(&opened)) return reject(L"A required file could not be closed.");
    return TRUE;
}
static BOOLEAN match_hash(hash_state *s, const UINT8 *expected) {
    UINT8 digest[32]; model_hash_final(s,digest); return same(digest,expected,32);
}
static BOOLEAN read_inputs(void) {
    for (UINTN i = 0; i < ARRAY_LEN(names); i++) {
        hash_state hash;
        if (!open_input(names[i]) || !read_bytes(inputs[i],sizes[i]) || !finish_file()) return FALSE;
        model_hash_init(&hash); model_hash_update(&hash,inputs[i],sizes[i]);
        if (!match_hash(&hash,pins[i])) {
            say(L"File: "); say(names[i]); say(L"\r\n");
            return reject(L"The file does not match the SHA-256 fixed in this loader.");
        }
    }
    for (UINTN i = 0; i < ARRAY_LEN(segments); i++)
        bs->CopyMem((void *)(UINTN)segments[i].address,inputs[0]+segments[i].offset,segments[i].size);
    return TRUE;
}

static CHAR16 lower(CHAR16 c) { return c >= L'A' && c <= L'Z' ? c+(L'a'-L'A') : c; }
static UINT64 shard_length(UINTN i) { return minimum(SHARD_BYTES, MODEL_BYTES-i*SHARD_BYTES); }
static BOOLEAN inventory(void) {
    UINT64 storage[128];
    for (;;) {
        UINTN size = sizeof(storage);
        if (EFI_ERROR(folder->Read(folder,&size,storage))) return reject(L"The PCPOS directory could not be read.");
        if (!size) break;
        EFI_FILE_INFO *entry = (EFI_FILE_INFO *)storage;
        if (size < SIZE_OF_EFI_FILE_INFO + sizeof(CHAR16) || entry->Size > size
                || entry->Size < SIZE_OF_EFI_FILE_INFO + sizeof(CHAR16))
            return reject(L"The boot volume returned an invalid directory entry.");
        UINTN room = (entry->Size-SIZE_OF_EFI_FILE_INFO)/sizeof(CHAR16), n = 0;
        while (n < room && entry->FileName[n]) n++;
        if (n == room) return reject(L"The boot volume returned an incomplete file name.");
        const CHAR16 *prefix = L"shard-";
        UINTN j = 0;
        while (j < n && prefix[j] && lower(entry->FileName[j]) == prefix[j]) j++;
        if (j != 6) continue;
        if (n != 15 || entry->FileName[11] != L'.' || lower(entry->FileName[12]) != L'b'
                || lower(entry->FileName[13]) != L'i' || lower(entry->FileName[14]) != L'n')
            return reject(L"A shard in PCPOS has an unexpected name.");
        UINTN index = 0;
        for (j = 6; j < 11; j++) {
            CHAR16 c = entry->FileName[j];
            if (c < L'0' || c > L'9') return reject(L"A shard number is not valid.");
            index = index*10 + c-L'0';
        }
        if (index >= SHARD_COUNT || (seen_shards[index/8] & (1U << (index%8))))
            return reject(L"The volume has an extra or duplicate model shard.");
        if ((entry->Attribute & EFI_FILE_DIRECTORY) || entry->FileSize != shard_length(index))
            return reject(L"A model shard has the wrong length or is not a file.");
        seen_shards[index/8] |= (UINT8)(1U << (index%8));
    }
    for (UINTN i = 0; i < SHARD_COUNT; i++)
        if (!(seen_shards[i/8] & (1U << (i%8)))) return reject(L"The volume is missing a model shard.");
    return TRUE;
}

static BOOLEAN read_model(void) {
    hash_state whole; model_hash_init(&whole);
    UINTN pi = 0; UINT64 offset = 0, total = 0;
    for (UINTN i = 0; i < SHARD_COUNT; i++) {
        CHAR16 name[] = L"shard-00000.bin";
        UINTN ordinal = i;
        for (int j = 10; j >= 6; j--) { name[j] = L'0'+ordinal%10; ordinal /= 10; }
        if (!open_input(name)) return FALSE;
        hash_state shard; model_hash_init(&shard);
        UINT64 left = shard_length(i);
        while (left) {
            if (pi >= piece_count) return reject(L"The model does not fit the reserved memory.");
            UINTN take = (UINTN)minimum(minimum(left,IO_BLOCK),pieces[pi].size-offset);
            UINT8 *where = (UINT8 *)(UINTN)(pieces[pi].start+offset);
            if (!read_bytes(where,take)) return FALSE;
            model_hash_update(&shard,where,take);
            model_hash_update(&whole,where,take);
            left -= take; offset += take; total += take;
            if (offset == pieces[pi].size) { pi++; offset = 0; }
        }
        if (!finish_file()) return FALSE;
        if (!match_hash(&shard,inputs[3]+56+32*i)) {
            say(L"File: "); say(name); say(L"\r\n");
            return reject(L"A model shard does not match its ordered manifest digest.");
        }
        say(L"Verified "); number(total); say(L" of "); number(MODEL_BYTES); say(L" model bytes.\r\n");
    }
    if (total != MODEL_BYTES || !match_hash(&whole,inputs[3]+24))
        return reject(L"The complete model does not match the pinned manifest.");
    UINTN tail = page_size(total)-total;
    if (tail) {
        if (pi+1 != piece_count || pieces[pi].size-offset != tail)
            return reject(L"The final model page has unexpected bounds.");
        bs->SetMem((void *)(UINTN)(pieces[pi].start+offset),tail,0);
    }
    return TRUE;
}

struct line { char *text; UINTN used; };
static BOOLEAN append(struct line *l, const char *s) {
    while (*s) {
        if (l->used+1 >= sizeof(boot->command)) return reject(L"The kernel command line is too long.");
        l->text[l->used++] = *s++;
    }
    l->text[l->used] = 0; return TRUE;
}
static BOOLEAN append_hex(struct line *l, UINT64 number_value) {
    static const char digits[] = "0123456789abcdef";
    char buffer[19]; char *p = buffer+sizeof(buffer)-1; *p = 0;
    do { *--p = digits[number_value&15]; number_value >>= 4; } while (number_value);
    *--p = 'x'; *--p = '0'; return append(l,p);
}
static void make_module(UINTN i, UINTN input, const char *name) {
    UINTN n = 0;
    while (name[n]) { boot->names[i][n] = name[n]; n++; }
    boot->names[i][n] = 0;
    boot->mods[i] = (struct mb_module){(UINT32)(UINTN)inputs[input],(UINT32)((UINTN)inputs[input]+sizes[input]),
                                      (UINT32)(UINTN)boot->names[i],0};
}
struct __attribute__((packed)) framebuffer {
    UINT64 address;
    UINT32 pitch, width, height;
    UINT8 depth, kind, red, red_bits, green, green_bits, blue, blue_bits;
};
_Static_assert(sizeof(struct framebuffer)==28, "Multiboot framebuffer");
static void console_framebuffer(void) {
    EFI_GUID guid = EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID;
    EFI_GRAPHICS_OUTPUT_PROTOCOL *gop = NULL;
    if (EFI_ERROR(bs->HandleProtocol(st->ConsoleOutHandle,&guid,(void **)&gop)) || !gop || !gop->Mode) return;
    EFI_GRAPHICS_OUTPUT_MODE_INFORMATION *info = gop->Mode->Info;
    if (!info || gop->Mode->SizeOfInfo < sizeof(*info) || info->PixelFormat != PixelBlueGreenRedReserved8BitPerColor
            || !info->HorizontalResolution || !info->VerticalResolution || info->PixelsPerScanLine < info->HorizontalResolution
            || info->PixelsPerScanLine > 0xffffffffU/4) return;
    UINT32 pitch = info->PixelsPerScanLine*4;
    UINT64 need = (UINT64)pitch*info->VerticalResolution;
    UINT64 fb = gop->Mode->FrameBufferBase;
    if (!fb || fb%4 || need > gop->Mode->FrameBufferSize || fb > (~(UINT64)0)-need) return;
    struct framebuffer f = {fb,pitch,info->HorizontalResolution,info->VerticalResolution,32,1,16,8,8,8,0,8};
    bs->CopyMem((UINT8 *)boot->info+88,&f,sizeof(f));
    boot->info[0] |= 1U<<12;
}
static BOOLEAN prepare_handoff(void) {
    make_module(0,1,"tailorbird"); make_module(1,2,"model-worker"); make_module(2,3,"model.manifest");
    boot->info[0] = 4|8|0x40; boot->info[4] = (UINT32)(UINTN)boot->command;
    boot->info[5] = 3; boot->info[6] = (UINT32)(UINTN)boot->mods; boot->info[12] = (UINT32)(UINTN)boot->ranges;
    console_framebuffer();
    struct line l = {boot->command,0};
    if (!append(&l,"image=" MEASUREMENT_TEXT " auto=tailorbird then=menu propose_image=" UI_TEXT " worker_for=" UI_TEXT
                   " worker_image=" WORKER_TEXT " model_manifest=" PIN_TEXT " model_memory=")) return FALSE;
    UINT64 left = MODEL_BYTES;
    for (UINTN i = 0; i < piece_count; i++) {
        UINT64 logical = minimum(left,pieces[i].size);
        if ((i && !append(&l,",")) || !append_hex(&l,pieces[i].start) || !append(&l,":") || !append_hex(&l,logical)) return FALSE;
        left -= logical;
    }
    return left == 0 || reject(L"The kernel model command line is incomplete.");
}
static BOOLEAN add_range(UINT64 a, UINT64 b, UINT32 type) {
    if (range_count && boot->ranges[range_count-1].type == type) {
        struct mb_range *r = &boot->ranges[range_count-1];
        if (r->base+r->length == a) { r->length += b-a; return TRUE; }
    }
    if (range_count >= MAP_ENTRIES) return reject(L"The final firmware map exceeds the kernel map limit.");
    boot->ranges[range_count++] = (struct mb_range){20,a,b-a,type}; return TRUE;
}
static BOOLEAN translate(void) {
    UINT64 coverage[MAX_PIECES] = {0};
    range_count = 0;
    for (UINTN i = 0; i < map_count; i++) {
        EFI_MEMORY_DESCRIPTOR *d = descriptor(i);
        UINT64 a = d->PhysicalStart, end = end_of(d);
        while (a < end) {
            UINT64 next = end;
            UINT32 type = conventional(d) ? 1 : d->Type == EfiACPIMemoryNVS ? 4 : 2;
            for (UINTN j = 0; j < piece_count; j++) {
                UINT64 lo = pieces[j].start, hi = lo+pieces[j].size;
                if (a < lo) next = minimum(next,lo);
                else if (a < hi) {
                    if (d->Type != EfiLoaderData || !ordinary_ram(d))
                        return reject(L"A model range is not owned by this loader in the final map.");
                    next = minimum(next,hi);
                    type = MODEL_RAM;
                    break;
                }
            }
            if (type == MODEL_RAM) {
                for (UINTN j = 0; j < piece_count; j++)
                    if (a >= pieces[j].start && a < pieces[j].start+pieces[j].size) { coverage[j] += next-a; break; }
            }
            if (!add_range(a,next,type)) return FALSE;
            a = next;
        }
    }
    for (UINTN j = 0; j < piece_count; j++)
        if (coverage[j] != pieces[j].size) return reject(L"The final firmware map does not cover the model.");
    boot->info[11] = range_count*sizeof(struct mb_range);
    return TRUE;
}

static BOOLEAN release_page(UINTN index) {
    struct piece p = owned[index];
    if (EFI_ERROR(bs->FreePages(p.start,p.size/PAGE_BYTES))) return FALSE;
    owned[index] = owned[--owned_count]; return TRUE;
}
static BOOLEAN release_staging(void) {
    for (UINTN i = 0; i < owned_count; i++)
        if (owned[i].start == (UINTN)inputs[0]) {
            if (!release_page(i)) break;
            inputs[0] = NULL; return TRUE;
        }
    return reject(L"Firmware did not release the temporary kernel input buffer.");
}
static EFI_STATUS finish_error(void) {
    BOOLEAN released = TRUE;
    if (!exit_attempted) {
        if (!close_handle(&opened)) released=FALSE;
        if (!close_handle(&folder)) released=FALSE;
        if (!close_handle(&root)) released=FALSE;
    }
    while (owned_count) {
        UINTN last = owned_count-1;
        if (!release_page(last)) { owned_count--; released=FALSE; }
    }
    if (!exit_attempted) {
        say(L"PCP OS could not start. "); say(reason); say(L"\r\n");
        if (released) say(L"Loader resources released.\r\n");
        else say(L"Firmware reported an error while releasing resources.\r\n");
    }
    return released ? EFI_LOAD_ERROR : EFI_DEVICE_ERROR;
}
static BOOLEAN leave_firmware(EFI_HANDLE image) {
    for (UINTN attempt = 0; attempt < 8; attempt++) {
        if (!read_map() || !translate()) return FALSE;
        /* No firmware calls between this snapshot and ExitBootServices. After an
         * unsuccessful exit, only memory services are used to refresh the map. */
        exit_attempted = TRUE;
        EFI_STATUS status = bs->ExitBootServices(image,map_key);
        if (status == EFI_SUCCESS) return TRUE;
        if (status != EFI_INVALID_PARAMETER) break;
    }
    return reject(L"Firmware could not complete the handoff.");
}
EFI_STATUS efi_main(EFI_HANDLE image, EFI_SYSTEM_TABLE *system) {
    InitializeLib(image,system);
    st=system; bs=system->BootServices;
    if (EFI_ERROR(bs->SetWatchdogTimer(0,0,0,NULL))) {
        reject(L"Firmware could not disable the boot watchdog for model verification."); return finish_error();
    }
    if (!reserve_all() || !open_volume(image) || !read_inputs() || !release_staging() || !inventory()
            || !read_model() || !prepare_handoff()) return finish_error();
    if (!close_handle(&folder) || !close_handle(&root)) {
        reject(L"The boot volume could not be closed."); return finish_error();
    }
    say(L"Model verified. Starting PCP OS.\r\n");
    if (!leave_firmware(image)) return finish_error();
    transfer((UINTN)boot,KERNEL_ENTRY,(UINTN)boot->stack+sizeof(boot->stack));
}
