# Starting another processor (feature "processors"; assembled with arch/others.rs).
# Another processor begins in 16-bit real mode at the first byte of a page below one mebibyte, with
# nothing set up. The few instructions below are copied to such a page (arch/others.rs), and take the
# processor to 64-bit mode on the kernel's own map; then they jump to ap_entry in the kernel proper.
# They use no stack and call nothing. Four words are corrected when the copy is made: three hold an
# offset from the start to which the page's address is added, and one receives the map's address.
.section .rodata
.balign 16
.global ap_tramp_start
.global ap_tramp_end
.global ap_fix_gdt
.global ap_fix_pm
.global ap_fix_cr3
.global ap_fix_lm
.code16
ap_tramp_start:
    cli
    cld
    movw %cs, %ax
    movw %ax, %ds
    lgdtl ap_tramp_gdt_ptr - ap_tramp_start
    movl %cr0, %eax
    andl $0x9FFFFFFF, %eax        # caches on: a processor comes out of reset with them off
    orl  $1, %eax                 # protected mode
    movl %eax, %cr0
    .byte 0x66, 0xEA              # far jump into the 32-bit code below
ap_fix_pm:
    .long ap_tramp_32 - ap_tramp_start
    .word 0x08
.code32
ap_tramp_32:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movl %cr4, %eax
    orl  $0x20, %eax              # physical address extension
    movl %eax, %cr4
    .byte 0xB8                    # movl $(the kernel's map), %eax
ap_fix_cr3:
    .long 0
    movl %eax, %cr3
    movl $0xC0000080, %ecx
    rdmsr
    orl  $0x900, %eax             # long mode, and no-execute
    wrmsr
    movl %cr0, %eax
    orl  $0x80000000, %eax        # paging. Write protection is turned on in ap_main, after the task register is loaded.
    movl %eax, %cr0
    .byte 0xEA                    # far jump into the 64-bit code below
ap_fix_lm:
    .long ap_tramp_64 - ap_tramp_start
    .word 0x18
.code64
ap_tramp_64:
    movabsq $ap_entry, %rax
    jmp  *%rax
.balign 8
ap_tramp_gdt:
    .quad 0
    .quad 0x00CF9A000000FFFF      # 0x08: 32-bit code
    .quad 0x00CF92000000FFFF      # 0x10: data
    .quad 0x00AF9A000000FFFF      # 0x18: 64-bit code
ap_tramp_gdt_ptr:
    .word 31
ap_fix_gdt:
    .long ap_tramp_gdt - ap_tramp_start
ap_tramp_end:

# Where the copy above arrives: in the kernel's code, in 64-bit mode, on the kernel's map, still without a
# stack. One processor is started at a time, so one word is enough to hand it its stack.
.section .text
.code64
.global ap_entry
ap_entry:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    xorl %eax, %eax
    movw %ax, %fs
    movw %ax, %gs
    movq ap_stack_top(%rip), %rsp
    xorq %rbp, %rbp
    call ap_main
1:  cli
    hlt
    jmp  1b

# Where another processor waits when it has nothing to do: interrupts on, halted. A message from
# another processor (vector 240) is what wakes it; the handler returns here.
.global ap_idle
.global ap_idle_end
ap_idle:
    sti
1:  hlt
    jmp  1b
ap_idle_end:

# The entry for a message from another processor, built like every other entry in boot.s.
.global isr_message
isr_message:
    pushq $0
    pushq $240
    jmp  isr_common

.section .data
.align 8
.global ap_stack_top
ap_stack_top: .quad 0
