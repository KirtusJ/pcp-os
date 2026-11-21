/* EfiLoaderCode is identity mapped on this x86-64 path. Both the transition
 * code and the supplied stack must have been checked to fit below 4 GiB. */
.text
.globl transfer
.type transfer,@function
transfer:
    cli
    cld
    mov %rdx,%rsp
    and $-16,%rsp
    sub $16,%rsp
    lea gdt(%rip),%rax
    movw $(gdt_end-gdt-1),(%rsp)
    mov %rax,2(%rsp)
    lgdt (%rsp)
    pushq $8
    lea compat(%rip),%rax
    push %rax
    lretq
.code32
compat:
    mov $16,%ax
    mov %ax,%ds
    mov %ax,%es
    mov %ax,%ss
    mov %ax,%fs
    mov %ax,%gs
    mov %cr0,%eax
    btr $31,%eax
    mov %eax,%cr0
    mov $0xc0000080,%ecx
    rdmsr
    btr $8,%eax
    wrmsr
    mov %cr4,%eax
    btr $5,%eax
    mov %eax,%cr4
    mov %edi,%ebx
    mov $0x2badb002,%eax
    jmp *%esi
.code64
.p2align 3
gdt:
    .quad 0
    .quad 0x00cf9a000000ffff
    .quad 0x00cf92000000ffff
gdt_end:
.globl transfer_end
transfer_end:
.section .note.GNU-stack,"",@progbits
