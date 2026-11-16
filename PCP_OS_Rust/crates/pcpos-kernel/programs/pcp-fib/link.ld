ENTRY(_start)
PHDRS { code PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS {
  . = 0x8000400000;
  .text : { *(.text .text.*) *(.rodata .rodata.*) } :code
  . = ALIGN(4096);
  .data : { *(.data .data.*) *(.bss .bss.*) *(.got .got.*) } :data
  /DISCARD/ : { *(.eh_frame*) *(.comment) *(.note*) }
}
