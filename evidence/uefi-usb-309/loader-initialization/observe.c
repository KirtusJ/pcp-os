/* Observe GNU-EFI InitializeLib on a host with two synthetic variable-store responses. */
#include <efi.h>
#include <efilib.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int supply_language, reads, writes, protocols, locates, allocations, frees;
static char order[2][32];

static EFI_STATUS EFIAPI read_variable(CHAR16 *name, EFI_GUID *guid, UINT32 *attrs,
                                       UINTN *size, void *data) {
    char text[32] = {0};
    size_t i = 0;
    while (name[i] && i + 1 < sizeof(text)) { text[i] = (char) name[i]; ++i; }
    if (name[i] || reads >= 2) abort();
    strcpy(order[reads++], text);
    if (strcmp(text, "Lang") || !supply_language) return EFI_NOT_FOUND;
    if (*size < 4) { *size = 4; return EFI_BUFFER_TOO_SMALL; }
    memcpy(data, "eng", 4);
    *size = 4;
    if (attrs) *attrs = EFI_VARIABLE_NON_VOLATILE;
    return EFI_SUCCESS;
}
static EFI_STATUS EFIAPI write_variable(CHAR16 *name, EFI_GUID *guid, UINT32 attrs,
                                        UINTN size, void *data) {
    ++writes;
    return EFI_WRITE_PROTECTED;
}
static EFI_STATUS EFIAPI handle(EFI_HANDLE h, EFI_GUID *guid, void **interface) {
    static EFI_LOADED_IMAGE image;
    if (memcmp(guid,&gEfiLoadedImageProtocolGuid,sizeof(*guid))) abort();
    ++protocols;
    image.ImageDataType = EfiLoaderData;
    *interface = &image;
    return EFI_SUCCESS;
}
static EFI_STATUS EFIAPI locate(EFI_LOCATE_SEARCH_TYPE search, EFI_GUID *guid,
                                void *key, UINTN *size, EFI_HANDLE *buffer) {
    if (search != ByProtocol || memcmp(guid,&gEfiUnicodeCollationProtocolGuid,sizeof(*guid))) abort();
    ++locates;
    return EFI_NOT_FOUND;
}
static EFI_STATUS EFIAPI allocate(EFI_MEMORY_TYPE type, UINTN size, void **p) {
    *p = malloc(size);
    if (!*p) return EFI_OUT_OF_RESOURCES;
    ++allocations;
    return EFI_SUCCESS;
}
static EFI_STATUS EFIAPI release(void *p) {
    free(p);
    ++frees;
    return EFI_SUCCESS;
}
int main(int argc, char **argv) {
    if (argc != 2 || (strcmp(argv[1],"missing") && strcmp(argv[1],"present"))) return 2;
    supply_language = !strcmp(argv[1],"present");
    EFI_BOOT_SERVICES bs = {.HandleProtocol=handle, .LocateHandle=locate,
                            .AllocatePool=allocate, .FreePool=release};
    EFI_RUNTIME_SERVICES rt = {.GetVariable=read_variable, .SetVariable=write_variable};
    EFI_SYSTEM_TABLE st = {.BootServices=&bs, .RuntimeServices=&rt};
    int token = 0;
    InitializeLib((EFI_HANDLE)&token, &st);
    printf("Lang=%s reads=%s,%s writes=%d loaded_image=%d unicode_locate=%d allocations=%d frees=%d\n",
           argv[1],order[0],order[1],writes,protocols,locates,allocations,frees);
    return !(reads==2 && !strcmp(order[0],"EFIDebug") && !strcmp(order[1],"Lang")
             && writes==0 && protocols==1 && locates==1 && allocations==frees);
}
