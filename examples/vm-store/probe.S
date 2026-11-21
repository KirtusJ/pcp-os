/* Freestanding x86-64 Linux ABI. Build each action as its own tiny ELF. */
.intel_syntax noprefix
.text
.global _start
_start:
    sub rsp, 32
#if ACTION == 1                    /* create, write and fsync old bytes */
    lea rdi, [rip+userfile]
    call open_write
    lea rsi, [rip+old_value]
    call write_value
    mov rdi, r12
    call sync_fd
#elif ACTION == 2 || ACTION == 4   /* verify old or replacement after boot */
    lea rdi, [rip+userfile]
    mov eax, 2
    xor esi, esi
    syscall
    test rax, rax
    js fail
    mov rdi, rax
    xor eax, eax
    mov rsi, rsp
    mov edx, 16
    syscall
    cmp eax, 8
    jne fail
#if ACTION == 2
    mov rax, qword ptr [rip+old_value]
#else
    mov rax, qword ptr [rip+new_value]
#endif
    cmp qword ptr [rsp], rax
    jne fail
#elif ACTION == 3 || ACTION == 5   /* replace and optionally fsync directory */
    lea rdi, [rip+staging]
    call open_write
    lea rsi, [rip+new_value]
    call write_value
    mov eax, 82
    lea rdi, [rip+staging]
    lea rsi, [rip+userfile]
    syscall
    test rax, rax
    jne fail
    mov eax, 82
    lea rdi, [rip+userfile]
    mov rsi, rdi
    syscall
    test rax, rax
    jne fail
#if ACTION == 3
    mov eax, 2
    lea rdi, [rip+directory]
    xor esi, esi
    syscall
    test rax, rax
    js fail
    mov rdi, rax
    call sync_fd
#endif
#elif ACTION == 6                   /* console-only program cannot checkpoint */
    lea rdi, [rip+directory]
    mov eax, 2
    xor esi, esi
    syscall
    test rax, rax
    js fail
    mov rdi, rax
    mov eax, 74
    syscall
    cmp rax, -13
    jne fail
#elif ACTION == 7                   /* larger than the volume, preserve old commit */
    lea rdi, [rip+staging]
    call open_write
    mov r13d, 9
large:
    mov rdi, r12
    lea rsi, [rip+large_block]
    mov edx, 8192
    mov eax, 1
    syscall
    cmp rax, 8192
    jne fail
    dec r13d
    jnz large
    mov eax, 74
    mov rdi, r12
    syscall
    cmp rax, -28
    jne fail
#elif ACTION == 8                   /* ordinary RAM file, no mounted checkpoint volume */
    lea rdi, [rip+userfile]
    call open_write
    lea rsi, [rip+old_value]
    call write_value
    mov eax, 74
    mov rdi, r12
    syscall
    cmp rax, -38                    /* ENOSYS */
    jne fail
#else
#error unknown ACTION
#endif
    mov eax, 1
    mov edi, 1
    lea rsi, [rip+success]
    mov edx, 15
    syscall
    mov eax, 60
    xor edi, edi
    syscall
fail:
    mov eax, 60
    mov edi, 42
    syscall
open_write:
    mov eax, 2
    mov esi, 577
    mov edx, 420
    syscall
    test rax, rax
    js fail
    mov r12, rax
    ret
write_value:
    mov eax, 1
    mov rdi, r12
    mov edx, 8
    syscall
    cmp rax, 8
    jne fail
    ret
sync_fd:
    mov eax, 74
    syscall
    test rax, rax
    jne fail
    ret
userfile: .asciz "guest-file"
staging: .asciz "replacement.tmp"
directory: .asciz "."
old_value: .ascii "oldbytes"
new_value: .ascii "newbytes"
success: .ascii "STORE PROBE OK\n"
#if ACTION == 7
large_block: .zero 8192
#endif
