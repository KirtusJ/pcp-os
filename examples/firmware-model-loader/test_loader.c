/* Host checks of the loader's actual memory, file and exit functions using EFI-shaped services. */
#include <assert.h>
#include <string.h>
#include <stdio.h>
#include "loader.c"

static EFI_BOOT_SERVICES services;
static EFI_SYSTEM_TABLE system_fixture;
static struct boot_data mb;
static EFI_MEMORY_DESCRIPTOR descriptors[12];
static UINTN descriptor_count, allocate_calls, free_calls, allocated, reject_at, closes, reads, map_calls, exit_calls;
static EFI_FILE_PROTOCOL file_fixture;
static UINT8 content[8192];
static UINTN file_len, position, read_limit, fail_read;
static BOOLEAN stale_once;
static EFI_STATUS EFIAPI fake_map(UINTN *length, EFI_MEMORY_DESCRIPTOR *map, UINTN *key, UINTN *step, UINT32 *version) {
    UINTN need=descriptor_count*sizeof(*map);
    assert(*length>=need);
    memcpy(map,descriptors,need); *length=need; *step=sizeof(*map); *version=EFI_MEMORY_DESCRIPTOR_VERSION;
    *key=++map_calls; return EFI_SUCCESS;
}
static EFI_STATUS EFIAPI fake_alloc(EFI_ALLOCATE_TYPE t, EFI_MEMORY_TYPE type, UINTN count, EFI_PHYSICAL_ADDRESS *address) {
    assert(t==AllocateAddress && type==EfiLoaderData && count && *address%PAGE_BYTES==0);
    if (++allocate_calls==reject_at) return EFI_OUT_OF_RESOURCES;
    allocated++; return EFI_SUCCESS;
}
static EFI_STATUS EFIAPI fake_free(EFI_PHYSICAL_ADDRESS address, UINTN pages) {
    assert(address && pages && allocated); allocated--; free_calls++; return EFI_SUCCESS;
}
static EFI_STATUS EFIAPI fake_read(EFI_FILE_PROTOCOL *f, UINTN *length, void *buf) {
    assert(f==&file_fixture);
    if (++reads==fail_read) return EFI_DEVICE_ERROR;
    UINTN count=minimum(*length,minimum(read_limit,file_len-position));
    memcpy(buf,content+position,count); position+=count; *length=count; return EFI_SUCCESS;
}
static EFI_STATUS EFIAPI fake_close(EFI_FILE_PROTOCOL *f) { assert(f==&file_fixture); closes++; return EFI_SUCCESS; }
static EFI_STATUS EFIAPI fake_exit(EFI_HANDLE image, UINTN key) {
    assert(image==(void *)1 && key==map_calls && key==exit_calls+1);
    exit_calls++; return stale_once && exit_calls==1 ? EFI_INVALID_PARAMETER : EFI_SUCCESS;
}
static EFI_MEMORY_DESCRIPTOR map_entry(UINT32 type, UINT64 addr, UINT64 bytes) {
    EFI_MEMORY_DESCRIPTOR d = {0}; d.Type=type; d.PhysicalStart=addr; d.NumberOfPages=bytes/PAGE_BYTES;
    d.Attribute=EFI_MEMORY_WB; return d;
}
static void reset(void) {
    memset(&services,0,sizeof(services)); memset(&system_fixture,0,sizeof(system_fixture));
    memset(descriptors,0,sizeof(descriptors)); memset(pieces,0,sizeof(pieces)); memset(owned,0,sizeof(owned));
    descriptor_count=allocate_calls=free_calls=allocated=reject_at=closes=reads=map_calls=exit_calls=0;
    owned_count=piece_count=range_count=0; exit_attempted=FALSE; stale_once=FALSE;
    bs=&services; st=&system_fixture; boot=&mb; root=folder=opened=NULL;
    services.GetMemoryMap=fake_map; services.AllocatePages=fake_alloc; services.FreePages=fake_free;
    services.ExitBootServices=fake_exit; file_fixture.Read=fake_read; file_fixture.Close=fake_close;
}
static void fragments(void) {
    reset();
    const UINT64 pages=page_size(MODEL_BYTES)/PAGE_BYTES;
    assert(pages>3);
    descriptors[0]=map_entry(EfiConventionalMemory,HIGH_START,2*PAGE_BYTES);
    descriptors[1]=map_entry(EfiConventionalMemory,HIGH_START+16*PAGE_BYTES,(pages-2)*PAGE_BYTES);
    descriptors[2]=map_entry(EfiConventionalMemory,HIGH_END,8*PAGE_BYTES); /* outside kernel window */
    descriptors[3]=map_entry(EfiConventionalMemory,HIGH_START+8*PAGE_BYTES,4*PAGE_BYTES);
    descriptors[3].Attribute |= EFI_MEMORY_RUNTIME;
    descriptor_count=4;
}
static void restricted_ranges(void) {
    static const UINT64 attributes[] = {
        0, EFI_MEMORY_WT, EFI_MEMORY_WB | EFI_MEMORY_RUNTIME, EFI_MEMORY_WB | EFI_MEMORY_SP,
        EFI_MEMORY_WB | EFI_MEMORY_NV, EFI_MEMORY_WB | EFI_MEMORY_RP, EFI_MEMORY_WB | EFI_MEMORY_WP,
        EFI_MEMORY_WB | EFI_MEMORY_RO, EFI_MEMORY_WB | EFI_MEMORY_XP
    };
    const UINT64 size = page_size(MODEL_BYTES);
    for (UINTN i = 0; i < ARRAY_LEN(attributes); i++) {
        reset();
        descriptors[0] = map_entry(EfiConventionalMemory, HIGH_START, size);
        descriptors[1] = map_entry(EfiConventionalMemory, KERNEL_BASE, KERNEL_END-KERNEL_BASE);
        descriptors[0].Attribute = descriptors[1].Attribute = attributes[i];
        descriptor_count = 2;
        UINT8 *low = NULL;
        assert(!reserve_high() && !reserve_low(KERNEL_BASE, PAGE_BYTES, &low));
        assert(!allocate_calls && !reads && !low);
        assert(translate() && range_count == 2);
        assert(boot->ranges[0].type == 2 && boot->ranges[1].type == 2);

        descriptors[2] = map_entry(EfiConventionalMemory, HIGH_START + size, size);
        descriptors[2].Attribute |= EFI_MEMORY_UC | EFI_MEMORY_WT | EFI_MEMORY_WC;
        descriptor_count++;
        assert(reserve_high() && piece_count == 1 && pieces[0].start == descriptors[2].PhysicalStart);
        descriptors[2].Type = EfiLoaderData;
        assert(read_map() && translate());
        descriptors[2].Attribute = attributes[i];
        assert(read_map() && !translate());
    }
}
/* This case isolates byte placement. The OVMF volume checks use the real Rust SHA-256. */
void model_hash_init(hash_state *h) { memset(h,0,sizeof(*h)); }
void model_hash_update(hash_state *h, const UINT8 *b, UINTN n) { (void)b; h->words[0]+=n; }
void model_hash_final(hash_state *h, UINT8 *out) { assert(h->words[0]); memset(out,0,32); }
static EFI_STATUS EFIAPI open_shard(EFI_FILE_PROTOCOL *dir, EFI_FILE_HANDLE *result, CHAR16 *name, UINT64 mode, UINT64 attr) {
    (void)dir; assert(mode==EFI_FILE_MODE_READ && !attr && name[10]>=L'0' && name[10]<=L'1');
    UINTN id=name[10]-L'0'; position=0; file_len=shard_length(id);
    for (UINTN i=0;i<file_len;i++) content[i]=(UINT8)((SHARD_BYTES*id+i)%251);
    *result=&file_fixture; return EFI_SUCCESS;
}
static void EFIAPI set_memory(void *where, UINTN bytes, UINT8 value) { memset(where,value,bytes); }
static void placement(void) {
    static UINT8 left[4096+2],right[3*4096+2],manifest[120];
    static EFI_FILE_PROTOCOL directory;
    reset(); directory.Open=open_shard; folder=&directory; bs->SetMem=set_memory;
    inputs[3]=manifest; read_limit=317;
    memset(left,0xa5,sizeof(left)); memset(right,0xa5,sizeof(right));
    pieces[0]=(struct piece){(UINTN)(left+1),4096};
    pieces[1]=(struct piece){(UINTN)(right+1),3*4096}; piece_count=2;
    assert(read_model() && closes==2);
    for (UINTN i=0;i<MODEL_BYTES;i++) assert((i<4096?left[i+1]:right[i-4096+1])==i%251);
    for (UINTN i=MODEL_BYTES-4096;i<3*4096;i++) assert(right[i+1]==0);
    assert(left[0]==0xa5 && left[sizeof(left)-1]==0xa5 && right[0]==0xa5 && right[sizeof(right)-1]==0xa5);
}
int main(void) {
    fragments(); assert(reserve_high()); assert(piece_count==2 && allocated==2);
    descriptors[0].Type=EfiLoaderData; descriptors[1].Type=EfiLoaderData;
    assert(read_map() && translate()); UINT64 marked=0;
    for (UINTN i=0;i<range_count;i++) if (boot->ranges[i].type==MODEL_RAM) marked+=boot->ranges[i].length;
    assert(marked==page_size(MODEL_BYTES));
    reason=L"host test"; assert(finish_error()==EFI_LOAD_ERROR && allocated==0 && free_calls==2);
    fragments(); reject_at=2; assert(!reserve_high() && allocated==1);
    assert(finish_error()==EFI_LOAD_ERROR && free_calls==1 && !allocated);
    fragments(); descriptors[1].NumberOfPages--; assert(!reserve_high() && !reads && !allocate_calls);
    reset(); opened=&file_fixture; file_len=sizeof(content); position=0; read_limit=3; fail_read=0;
    for (UINTN i=0;i<file_len;i++) content[i]=(UINT8)i;
    UINT8 copy[sizeof(content)]; assert(read_bytes(copy,sizeof(copy)) && !memcmp(copy,content,sizeof(copy)));
    assert(finish_file() && closes==1 && !opened);
    opened=&file_fixture; position=0; reads=0; fail_read=2;
    assert(!read_bytes(copy,sizeof(copy)) && position==3);
    assert(finish_error()==EFI_LOAD_ERROR && !opened && closes==2);
    opened=&file_fixture; fail_read=0; position=0; file_len=3;
    assert(!read_bytes(copy,4)); position=0; assert(read_bytes(copy,2) && !finish_file());
    reset(); descriptors[0]=map_entry(EfiConventionalMemory,0x200000,0x200000); descriptor_count=1;
    stale_once=TRUE; assert(leave_firmware((void *)1));
    assert(exit_calls==2 && map_calls==2 && boot->info[11]==sizeof(struct mb_range));
    placement();
    restricted_ranges();
    puts("PASS data across a physical split, exact zero page tail and intact sentinels; fragmented allocation, final ownership, tail coverage, full and partial rollback, RAM refusal before reads, short reads, I/O errors, EOF, stale map key/retry");
    return 0;
}
