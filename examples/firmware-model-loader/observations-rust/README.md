# Rust loader and sequential-prefill worker on read-only USB

The reproducible loader from `3243828ea27f167e8f09db6afcd012d598fb184c` started Tailorbird #22 on the combined #371 kernel. Two separate Linux checkouts built the same 354,304-byte EFI from these inputs. The firmware read 491,400,064 verified Coder model bytes from eight shards. The native worker admitted the weights and tokenizer and completed its forward preflight before reporting Ready.

The six frames and serial transcript come from one boot. The user typed `Hello`, stopped the first request before any text arrived, cleared the conversation, and sent a second `Hello`. System Stop ended both processes and returned to the 1920×1080 desktop. F10 and Enter finished 72 guest checks with QEMU exit 33. The low allocator returned 251,808 of 251,808 issued frames and the high allocator returned 14,169 of 14,169. This run evaluates startup, cancellation and desktop return. Generation and time to first text are measured separately.

`receipt.json` binds each capture, the input receipt, the image and the model. `toolchain.json` records the host tools, firmware libraries and effective build profile used here. The nine files under `refusals/` are separate cold boots of private control volumes using the same EFI.
