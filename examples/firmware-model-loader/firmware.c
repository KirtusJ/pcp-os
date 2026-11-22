/* GNU-EFI ABI adapters. The Rust caller owns handles and allocation lifetimes. */
#include <efi.h>
#include <efilib.h>

static EFI_SYSTEM_TABLE *system_table;
static EFI_BOOT_SERVICES *services;
static EFI_HANDLE image_handle;
extern EFI_STATUS loader_main(void);

EFI_STATUS efi_main(EFI_HANDLE image, EFI_SYSTEM_TABLE *system) {
    InitializeLib(image, system);
    image_handle = image;
    system_table = system;
    services = system->BootServices;
    return loader_main();
}
EFI_STATUS fw_watchdog(void) { return services->SetWatchdogTimer(0, 0, 0, NULL); }
void fw_print(CHAR16 *line) {
    if (system_table->ConOut) system_table->ConOut->OutputString(system_table->ConOut, line);
}
EFI_STATUS fw_root(EFI_FILE_HANDLE *root) {
    EFI_LOADED_IMAGE *app = NULL;
    EFI_FILE_IO_INTERFACE *volume = NULL;
    EFI_STATUS s = services->HandleProtocol(image_handle, &LoadedImageProtocol, (void **)&app);
    if (EFI_ERROR(s) || !app) return EFI_DEVICE_ERROR;
    s = services->HandleProtocol(app->DeviceHandle, &FileSystemProtocol, (void **)&volume);
    if (EFI_ERROR(s) || !volume) return EFI_DEVICE_ERROR;
    return volume->OpenVolume(volume, root);
}
EFI_STATUS fw_open(EFI_FILE_HANDLE parent, CHAR16 *name, EFI_FILE_HANDLE *out) {
    return parent->Open(parent, out, name, EFI_FILE_MODE_READ, 0);
}
EFI_STATUS fw_read(EFI_FILE_HANDLE file, UINTN *count, void *data) {
    return file->Read(file, count, data);
}
EFI_STATUS fw_close(EFI_FILE_HANDLE file) { return file->Close(file); }
EFI_STATUS fw_map(UINTN *len, void *bytes, UINTN *key, UINTN *step, UINT32 *version) {
    return services->GetMemoryMap(len, bytes, key, step, version);
}
EFI_STATUS fw_claim(UINTN pages, EFI_PHYSICAL_ADDRESS *address) {
    return services->AllocatePages(AllocateAddress, EfiLoaderData, pages, address);
}
EFI_STATUS fw_free(EFI_PHYSICAL_ADDRESS start, UINTN pages) {
    return services->FreePages(start, pages);
}
EFI_STATUS fw_exit(UINTN key) { return services->ExitBootServices(image_handle, key); }
UINT64 fw_acpi_table(UINT32 modern) {
    EFI_GUID v2 = ACPI_20_TABLE_GUID, v1 = ACPI_TABLE_GUID;
    VOID *table = NULL;
    EFI_STATUS status = LibGetSystemConfigurationTable(modern ? &v2 : &v1, &table);
    return EFI_ERROR(status) ? 0 : (UINT64)(UINTN)table;
}
struct mode { UINT64 base, size; UINT32 width, height, stride, format; };
int fw_display(struct mode *out) {
    EFI_GUID guid = EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID;
    EFI_GRAPHICS_OUTPUT_PROTOCOL *gop = NULL;
    if (EFI_ERROR(services->HandleProtocol(system_table->ConsoleOutHandle, &guid,(void **)&gop))
            || !gop || !gop->Mode || !gop->Mode->Info
            || gop->Mode->SizeOfInfo < sizeof(EFI_GRAPHICS_OUTPUT_MODE_INFORMATION)) return 0;
    EFI_GRAPHICS_OUTPUT_MODE_INFORMATION *info = gop->Mode->Info;
    *out = (struct mode){gop->Mode->FrameBufferBase, gop->Mode->FrameBufferSize,
                         info->HorizontalResolution, info->VerticalResolution,
                         info->PixelsPerScanLine, info->PixelFormat};
    return 1;
}
