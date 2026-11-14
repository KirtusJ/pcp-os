ENTRY(_start)
PHDRS {
 code PT_LOAD FLAGS(5);
 ro PT_LOAD FLAGS(4);
 rw PT_LOAD FLAGS(6);
}
SECTIONS {
 . = 0x8000400000;
 .text : { *(.text .text.* .ltext .ltext.*) } :code
 . = ALIGN(4096);
 .rodata : { *(.rodata .rodata.* .lrodata .lrodata.*) *(.eh_frame*) } :ro
 . = ALIGN(4096);
 .data : { *(.data .data.* .ldata .ldata.*) *(.got .got.*) } :rw
 .bss : { *(.bss .bss.* .lbss .lbss.*) *(COMMON) } :rw
 /DISCARD/ : { *(.note*) *(.comment*) }
}
