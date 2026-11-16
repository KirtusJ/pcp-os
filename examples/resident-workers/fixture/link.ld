ENTRY(_start)
PHDRS {
    code PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}
SECTIONS {
    . = 0x8000400000;
    .text : {
        *(.text .text.* .ltext .ltext.*)
        *(.rodata .rodata.* .lrodata .lrodata.*)
    } :code
    . = ALIGN(4096);
    .data : { *(.data .data.* .ldata .ldata.* .got .got.*) } :data
    .bss (NOLOAD) : { *(.bss .bss.* .lbss .lbss.* COMMON) } :data
    /DISCARD/ : { *(.eh_frame* .comment .note*) }
}
