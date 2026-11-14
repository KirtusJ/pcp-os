/* The kernel is loaded at 1 MiB and runs there. Sections are page-aligned so that each can be mapped with
   exactly the permissions it needs: code is never writable, data is never executable. */
ENTRY(_start)
SECTIONS
{
  . = 1M;
  .boot : { KEEP(*(.multiboot)) *(.boot.text) *(.boot.data) }
  .text : ALIGN(4K) { __text_start = .; *(.text .text.*) . = ALIGN(4K); __text_end = .; }
  .rodata : ALIGN(4K) { __rodata_start = .; *(.rodata .rodata.*) . = ALIGN(4K); __rodata_end = .; }
  .data : ALIGN(4K) { __data_start = .; *(.data .data.*) }
  .bss : ALIGN(4K) { *(.bss .bss.*) *(COMMON) . = ALIGN(4K); __kernel_end = .; }
  /DISCARD/ : { *(.eh_frame*) *(.note*) *(.comment) }
}
