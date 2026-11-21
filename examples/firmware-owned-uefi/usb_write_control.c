/* QEMU TCG diagnostic only, separate from PCP OS and its boot image. */
#include <efi.h>
#include <efilib.h>
#include <efipciio.h>
#include <cpuid.h>

static EFI_GUID partition = {0x32802589,0xa96c,0x4a24,{0x8f,0xa4,0xde,0x34,0x9d,0x70,0xcb,0x05}};
static EFI_GUID vendor = {0x86a1bff3,0x4b7c,0x4860,{0x99,0xda,0x10,0x56,0xde,0x89,0xab,0xaa}};
static const char marker[] = "PCP OS disk-preservation test. Keep this file across cold boots.\r\n";
static const char value[] = "vm-scratch-marker-v1";

static void report(const char *s) {
    for (; *s; s++) __asm__ volatile ("outb %0, $0xe9" : : "a" (*s));
}

static BOOLEAN same(const void *first, const void *second, UINTN n) {
    const UINT8 *a = first, *b = second;
    for (UINTN i = 0; i < n; i++) if (a[i] != b[i]) return FALSE;
    return TRUE;
}

static BOOLEAN is_tcg(void) {
    UINT32 a, b, c, d;
    __cpuid(1, a, b, c, d);
    if (!(c & (1u << 31))) return FALSE;
    __cpuid(0x40000000, a, b, c, d);
    UINT32 name[] = {b, c, d};
    return same(name, "TCGTCGTCGTCG", 12);
}

static EFI_STATUS wait_key(EFI_SYSTEM_TABLE *st) {
    EFI_INPUT_KEY key;
    UINTN event;
    EFI_STATUS status = uefi_call_wrapper(st->BootServices->WaitForEvent, 3, 1, &st->ConIn->WaitForKey, &event);
    if (EFI_ERROR(status)) return status;
    return uefi_call_wrapper(st->ConIn->ReadKeyStroke, 2, st->ConIn, &key);
}

static EFI_STATUS connect_nvme(EFI_BOOT_SERVICES *bs) {
    EFI_GUID pci_guid = EFI_PCI_IO_PROTOCOL_GUID;
    EFI_HANDLE *handles = NULL, found = NULL;
    UINTN count = 0;
    EFI_STATUS status = uefi_call_wrapper(bs->LocateHandleBuffer, 5, ByProtocol, &pci_guid, NULL, &count, &handles);
    if (EFI_ERROR(status)) return status;
    for (UINTN i = 0; i < count; ++i) {
        EFI_PCI_IO_PROTOCOL *pci;
        UINT32 id, class_rev;
        if (EFI_ERROR(uefi_call_wrapper(bs->HandleProtocol, 3, handles[i], &pci_guid, (void **)&pci))) continue;
        if (EFI_ERROR(uefi_call_wrapper(pci->Pci.Read, 5, pci, EfiPciIoWidthUint32, 0, 1, &id))) continue;
        if (EFI_ERROR(uefi_call_wrapper(pci->Pci.Read, 5, pci, EfiPciIoWidthUint32, 8, 1, &class_rev))) continue;
        if ((id & 0xffff) == 0x1b36 && (class_rev >> 8) == 0x010802) {
            if (found) return EFI_NO_MAPPING;
            found = handles[i];
        }
    }
    if (!found) return EFI_NOT_FOUND;
    return uefi_call_wrapper(bs->ConnectController, 4, found, NULL, NULL, TRUE);
}

static BOOLEAN is_partition(EFI_DEVICE_PATH *node) {
    BOOLEAN nvme = FALSE;
    while (!IsDevicePathEnd(node)) {
        if (DevicePathType(node) == MESSAGING_DEVICE_PATH && DevicePathSubType(node) == 0x17) nvme = TRUE;
        if (DevicePathType(node) == MEDIA_DEVICE_PATH && DevicePathSubType(node) == MEDIA_HARDDRIVE_DP) {
            HARDDRIVE_DEVICE_PATH *hd = (HARDDRIVE_DEVICE_PATH *)node;
            return nvme && hd->PartitionNumber == 1 && hd->PartitionStart == 2048 && hd->PartitionSize == 16384 &&
                hd->MBRType == MBR_TYPE_EFI_PARTITION_TABLE_HEADER && hd->SignatureType == SIGNATURE_TYPE_GUID &&
                same(hd->Signature, &partition, 16);
        }
        node = NextDevicePathNode(node);
    }
    return FALSE;
}

static EFI_STATUS write_marker(EFI_BOOT_SERVICES *bs) {
    EFI_HANDLE *handles, target = NULL;
    UINTN count = 0;
    EFI_STATUS status = uefi_call_wrapper(bs->LocateHandleBuffer, 5, ByProtocol, &FileSystemProtocol, NULL, &count, &handles);
    if (EFI_ERROR(status)) return status;
    for (UINTN i = 0; i < count; ++i) {
        EFI_DEVICE_PATH *path;
        if (EFI_ERROR(uefi_call_wrapper(bs->HandleProtocol, 3, handles[i], &DevicePathProtocol, (void **)&path))) continue;
        if (is_partition(path)) {
            if (target) return EFI_NO_MAPPING;
            target = handles[i];
        }
    }
    if (!target) return EFI_NOT_FOUND;
    EFI_SIMPLE_FILE_SYSTEM_PROTOCOL *fs;
    EFI_FILE *root, *file;
    status = uefi_call_wrapper(bs->HandleProtocol, 3, target, &FileSystemProtocol, (void **)&fs);
    if (EFI_ERROR(status)) return status;
    status = uefi_call_wrapper(fs->OpenVolume, 2, fs, &root);
    if (EFI_ERROR(status)) return status;
    status = uefi_call_wrapper(root->Open, 5, root, &file, L"KEEP.TXT", EFI_FILE_MODE_READ | EFI_FILE_MODE_WRITE, 0);
    if (EFI_ERROR(status)) return status;
    char data[sizeof(marker)];
    UINTN size = sizeof(data);
    status = uefi_call_wrapper(file->Read, 3, file, &size, data);
    if (EFI_ERROR(status) || size != sizeof(marker)-1 || !same(data, marker, size)) return EFI_COMPROMISED_DATA;
    status = uefi_call_wrapper(file->SetPosition, 2, file, 0);
    if (EFI_ERROR(status)) return status;
    size = 1;
    data[0] = 'p';
    status = uefi_call_wrapper(file->Write, 3, file, &size, data);
    if (EFI_ERROR(status) || size != 1) return EFI_DEVICE_ERROR;
    status = uefi_call_wrapper(file->Flush, 1, file);
    if (EFI_ERROR(status)) return status;
    status = uefi_call_wrapper(file->Close, 1, file);
    if (EFI_ERROR(status)) return status;
    return uefi_call_wrapper(root->Close, 1, root);
}

static EFI_STATUS write_var(EFI_RUNTIME_SERVICES *rt) {
    CHAR16 *name = L"PcpUsbObserverControl";
    char data[sizeof(value)];
    UINTN size = sizeof(data);
    UINT32 attr = 0;
    EFI_STATUS status = uefi_call_wrapper(rt->GetVariable, 5, name, &vendor, &attr, &size, data);
    if (status != EFI_NOT_FOUND) return EFI_ACCESS_DENIED;
    status = uefi_call_wrapper(rt->SetVariable, 5, name, &vendor, 7, sizeof(value)-1, (void *)value);
    if (EFI_ERROR(status)) return status;
    size = sizeof(data);
    status = uefi_call_wrapper(rt->GetVariable, 5, name, &vendor, &attr, &size, data);
    if (EFI_ERROR(status) || attr != 7 || size != sizeof(value)-1 || !same(data, value, size)) return EFI_DEVICE_ERROR;
    return EFI_SUCCESS;
}

EFI_STATUS efi_main(EFI_HANDLE image, EFI_SYSTEM_TABLE *st) {
    (void)image;
    if (!is_tcg()) return EFI_UNSUPPORTED;
    report("CONTROL ready\n");
    if (EFI_ERROR(wait_key(st))) return EFI_DEVICE_ERROR;
    if (EFI_ERROR(connect_nvme(st->BootServices))) { report("CONTROL NVMe connection failed\n"); return EFI_DEVICE_ERROR; }
    if (EFI_ERROR(write_marker(st->BootServices))) { report("CONTROL file failed\n"); return EFI_DEVICE_ERROR; }
    if (EFI_ERROR(write_var(st->RuntimeServices))) { report("CONTROL variable failed\n"); return EFI_DEVICE_ERROR; }
    report("CONTROL verified\n");
    if (EFI_ERROR(wait_key(st))) return EFI_DEVICE_ERROR;
    uefi_call_wrapper(st->RuntimeServices->ResetSystem, 4, EfiResetShutdown, EFI_SUCCESS, 0, NULL);
    return EFI_SUCCESS;
}
