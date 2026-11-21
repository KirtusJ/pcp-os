#include <assert.h>
#include <stdio.h>
#include <string.h>
#include <efi.h>
#include <efilib.h>

static EFI_GRAPHICS_OUTPUT_PROTOCOL graphics;
static EFI_HANDLE console_handle = (EFI_HANDLE) 1;
static unsigned int calls;
static unsigned int fields;
static int protocol_present;

static EFI_STATUS EFIAPI handle_protocol(EFI_HANDLE handle, EFI_GUID *guid, void **out) {
    EFI_GUID expected = EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID;
    assert(handle == console_handle && memcmp(guid, &expected, sizeof(expected)) == 0);
    calls++;
    if (!protocol_present) return EFI_UNSUPPORTED;
    *out = &graphics;
    return EFI_SUCCESS;
}

static void value(const char *name, UINT64 val) {
    assert(val > 0);
    assert(strcmp(name, "console_framebuffer") == 0 || strcmp(name, "console_pitch") == 0);
    fields++;
}

#include "source.inc"

static void check(const char *label, EFI_GRAPHICS_PIXEL_FORMAT format,
                  unsigned int width, unsigned int height, unsigned int stride,
                  int present, int expected) {
    EFI_GRAPHICS_OUTPUT_MODE_INFORMATION info = {0};
    EFI_GRAPHICS_OUTPUT_PROTOCOL_MODE mode = {0};
    EFI_BOOT_SERVICES services = {0};
    EFI_SYSTEM_TABLE system = {0};
    struct boot_data boot = {0};
    calls = fields = 0;
    protocol_present = present;
    info.HorizontalResolution = width;
    info.VerticalResolution = height;
    info.PixelFormat = format;
    info.PixelsPerScanLine = stride;
    if (format == PixelBitMask) {
        info.PixelInformation.RedMask = 0x00ff0000;
        info.PixelInformation.GreenMask = 0x0000ff00;
        info.PixelInformation.BlueMask = 0x000000ff;
        info.PixelInformation.ReservedMask = 0xff000000;
    }
    mode.Info = &info;
    mode.SizeOfInfo = sizeof(info);
    mode.FrameBufferBase = format == PixelBltOnly ? 0 : 0x80000000;
    mode.FrameBufferSize = format == PixelBltOnly ? 0 : (UINTN)stride * 4 * height;
    graphics.Mode = &mode;
    services.HandleProtocol = handle_protocol;
    system.BootServices = &services;
    system.ConsoleOutHandle = console_handle;
    describe_console(&system, &boot);
    struct mb_framebuffer result = {0};
    memcpy(&result, (UINT8 *)boot.info + 88, sizeof(result));
    int described = (boot.info[0] & (1U << 12)) != 0;
    assert(described == expected && calls == 1);
    if (described) {
        assert(result.address == mode.FrameBufferBase && result.width == width && result.height == height);
        assert(result.pitch == 4 * stride && result.bits == 32 && result.type == 1 && fields == 2);
        assert(result.red_pos == 16 && result.green_pos == 8 && result.blue_pos == 0);
    } else {
        struct mb_framebuffer empty = {0};
        assert(fields == 0 && memcmp(&result,&empty,sizeof(empty)) == 0);
    }
    printf("%s,%u,%u,%u,%d,%u\n", label, width, height, stride, described, result.pitch);
}
int main(void) {
    puts("input,width,height,pixels_per_scanline,framebuffer_described,handed_off_pitch");
    check("BGR", PixelBlueGreenRedReserved8BitPerColor,1920,1080,1920,1,1);
    check("BGR padded", PixelBlueGreenRedReserved8BitPerColor,1920,1080,1984,1,1);
    check("RGB", PixelRedGreenBlueReserved8BitPerColor,1920,1080,1920,1,0);
    check("bit mask with BGR channel positions", PixelBitMask,1920,1080,1920,1,0);
    check("BLT only", PixelBltOnly,1920,1080,1920,1,0);
    check("no GOP on console handle", PixelBlueGreenRedReserved8BitPerColor,1920,1080,1920,0,0);
    check("alternate current BGR mode",PixelBlueGreenRedReserved8BitPerColor,1024,768,1024,1,1);
    return 0;
}
