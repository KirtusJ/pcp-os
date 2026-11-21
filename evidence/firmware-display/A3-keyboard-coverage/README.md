# Coverage when the keyboard demonstration disables replay

On the default d92f5f6 kernel, adding test=frame-cutoffs raised the final pass count from 72 to 73 and the allocation-cutoff check appeared. On the demonstrated no-replay path both boots ended with 59 passes. The optional allocation test was absent even when the kernel received the option.

comparison.json preserves the multiset difference between serial pass records for both sets of options. The repeated frame-recovery record occurs three times. The four initial guests reached their exit and serial result; the host reader then rejected the printed command line because QEMU prefixed the kernel pathname. Four fresh runs checked the complete suffix and the independent image measurement, preserved the exit codes, and produced the same multiset for each case. No physical controller was tested.
