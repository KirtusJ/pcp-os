# Branches with commits outside the default branch, November 2026

A catalogue only. Nothing here was removed, and this page proposes removing nothing: a branch can
hold a finding that never reached the default branch, and a date does not say otherwise. It turns a
long list of branch names into something a person can read, so that each contributor, or the rights
holder, can say which of their own are history (issue 368).

Measured on a complete (not shallow) history against the default branch at
`6b0002d9aa1d99e6c976cc44a6f612c2743cdb7b`, on 22 November 2026. Of 365 branches, one is the default, 114 hold no commit
outside it (81 of those are listed in [the other record](branches-removed-2026-11.md), none removed),
and the **250** below each hold at least one.

- **Tip** is the branch's head commit. **Date** is that commit's own date, which is not the day the
  branch was last pushed and says nothing of whether its contributor is still at work.
- **Ahead** is the number of commits reachable from the tip and not from the default branch.
- SHA-256 of the list in name order, each line a full tip, two spaces, the name and a line feed:
  `38e26c908486da93db72a152eef812c2b5e5e6780487d99ac097ce34e11e1204`.

To make it again: `git for-each-ref --no-merged <default> refs/remotes/origin`, and for each
`git rev-list --count <default>..<tip>`.

## By prefix

| Prefix | Branches | Oldest tip | Newest tip | Commits ahead, together |
| --- | ---: | --- | --- | ---: |
| `evidence` | 31 | 2026-11-16 | 2026-11-21 | 58 |
| `b3` | 21 | 2026-11-21 | 2026-11-22 | 115 |
| `audit` | 20 | 2026-11-13 | 2026-11-14 | 159 |
| `director-2` | 20 | 2026-11-16 | 2026-11-20 | 1767 |
| `codex` | 17 | 2026-11-13 | 2026-11-14 | 42 |
| `director-g` | 17 | 2026-11-13 | 2026-11-15 | 38 |
| `a3` | 14 | 2026-11-21 | 2026-11-22 | 88 |
| `session-a` | 14 | 2026-11-15 | 2026-11-20 | 1668 |
| `director-3` | 10 | 2026-11-16 | 2026-11-16 | 23 |
| `director-10` | 9 | 2026-11-16 | 2026-11-16 | 25 |
| `session-b` | 9 | 2026-11-15 | 2026-11-16 | 169 |
| `z3` | 7 | 2026-11-21 | 2026-11-22 | 214 |
| `director-9` | 6 | 2026-11-16 | 2026-11-16 | 12 |
| `perf` | 6 | 2026-11-13 | 2026-11-14 | 38 |
| `director-6` | 5 | 2026-11-16 | 2026-11-16 | 372 |
| `director-1` | 4 | 2026-11-16 | 2026-11-16 | 4 |
| `director-12` | 4 | 2026-11-16 | 2026-11-16 | 80 |
| `director-4` | 4 | 2026-11-16 | 2026-11-16 | 4 |
| `director-8` | 4 | 2026-11-16 | 2026-11-16 | 5 |
| `kernel` | 4 | 2026-11-13 | 2026-11-13 | 25 |
| `(no prefix)` | 3 | 2026-11-20 | 2026-11-20 | 476 |
| `director-11` | 3 | 2026-11-16 | 2026-11-16 | 59 |
| `director-13` | 3 | 2026-11-16 | 2026-11-16 | 53 |
| `c3` | 2 | 2026-11-22 | 2026-11-22 | 6 |
| `fix` | 2 | 2026-11-13 | 2026-11-14 | 9 |
| `integration` | 2 | 2026-11-13 | 2026-11-13 | 55 |
| `director-14` | 1 | 2026-11-16 | 2026-11-16 | 8 |
| `director-5` | 1 | 2026-11-16 | 2026-11-16 | 1 |
| `director-7` | 1 | 2026-11-16 | 2026-11-16 | 1 |
| `director-pt` | 1 | 2026-11-15 | 2026-11-15 | 4 |
| `docs` | 1 | 2026-11-15 | 2026-11-15 | 1 |
| `feat` | 1 | 2026-11-14 | 2026-11-14 | 23 |
| `games` | 1 | 2026-11-14 | 2026-11-14 | 4 |
| `pcp` | 1 | 2026-11-14 | 2026-11-14 | 1 |
| `session-c` | 1 | 2026-11-14 | 2026-11-14 | 2 |

## Every branch

```
47d68392ec4c6e447f2beb40da3462d6eaf1f172  2026-11-21     1  a3/331-whole-skipped-text
af55e0180e886bbe64c46ff63154e7fe07d3fca1  2026-11-21     1  a3/evidence-309-console-modes
3cf0f05d0317db426d3ece75562e8e2566267a70  2026-11-21    23  a3/evidence-309-stopped-count
ae570e63b8ee56402b16eb857f2d3ab976e2c0a7  2026-11-21     8  a3/evidence-311
ba91f8629a8765507780b9a3273827e3db87a33a  2026-11-21     5  a3/evidence-314-small-keys
07b604932060daa58535b2dc0802098b5e209341  2026-11-21     1  a3/evidence-323-parent-control
057f456afe4b91ec7dbfccc065fef6a897bea90a  2026-11-22    14  a3/evidence-396-pre-smp-2f7d9d43
477edeb7a1b46562774d01e3dfb7aaaab29019da  2026-11-22    13  a3/evidence-preloaded-first-boot
b7c537f5d58cac4020088d1badb93a83fc45679c  2026-11-22     4  a3/evidence-tailorbird32-8b-71c5ba6f
7f8031c474e1fbe932ef40bd854b166795e509cd  2026-11-22     1  a3/model-free-reference-355e5c13
2cfec7a16bf2739b5c4646b4c139313b3d5708b9  2026-11-21     2  a3/pin-released-specifications
9d16624db04b5996b4a7cb4787532738d9fce1ae  2026-11-22    12  a3/preloaded-inventory-gate-nov22
fdd746e7d81db2bccc487d13ff8258f4daa35d40  2026-11-22     2  a3/release-acceptance-readback
7675fa505a9f06fba63fd1e2186a3b292aa26805  2026-11-21     1  a3/stream-image-verification
64e1d809179e963cd840cbe194cf1c0016f3fe05  2026-11-13     4  audit/build22-clause-source-identity
55fb41e9a4537f6a9b70f05a94d904eec99456d8  2026-11-13    17  audit/build22-method-tree-verification
486ef28339118f2bd53f8cbddad61d836c636895  2026-11-13     4  audit/doom-sound-and-soak-negative-oracles
32562dcc974c6eb9aa9ee0768609127a0d27534a  2026-11-13     3  audit/fix-native-cli-identity-strict-values
8d4f1de1b890c7b4600912fa4a49f9172ea5d536  2026-11-13     6  audit/fix-native-module-add-lock-and-slot
19e54f137813a7f06a37a37b03f878378b946966  2026-11-13     2  audit/fix-native-nonmodule-removal
175aa5ac16c9cba0f10a7feeb0ada867590f82f1  2026-11-13     3  audit/fix-python-atomic-write-repair-lock
cf9f5fb44ebd12bf5288d894752b1f8eeb2ca851  2026-11-13     2  audit/fix-python-atomic-write-symlink
45d7ceee59590cf4240308402eadf66294e09096  2026-11-13     2  audit/fix-python-object-store-integrity
65e77b32debe96b1a4a161f792c5e350b81cb9c5  2026-11-13     2  audit/fix-python-register-identity-and-repair
a86251c6f14a19ebbdf2a7c436d09f5d5b4b9874  2026-11-13     2  audit/kernel-task-admission-oracle
e293516a47e2639941ac252d4c995a735e20c1cf  2026-11-13    19  audit/oversized-clause-exactness
4aedb4233dae396fa1d113487287145402c1cf3d  2026-11-13    19  audit/pinned-acr-module-selection
0ab4969605071c3f535744c49b47af7c8430de33  2026-11-13     2  audit/preflight-python-sealed-method-destinations
9f3afb193030d851053ecd10da5f2065eac24304  2026-11-13     8  audit/python-native-wire-record-grammar
1189d651270385e0c9a361e292fd41f6254488c1  2026-11-14    22  audit/qemu-cross-boot-capture
5d1376202f58e7ae372220a24cc669712e445352  2026-11-13    26  audit/refuse-linked-module-ancestors
499ba789d459df64fb47d32c10e7190faa2c6416  2026-11-13     5  audit/refuse-native-artifact-hardlink-overwrite
67e5f0e49c0c5e1c43acffd173ed396720f34895  2026-11-13     5  audit/safe-native-repair-sidecar
5fb13a664f9d68a01e32fc7201e21d8e51867bb4  2026-11-13     6  audit/serialize-native-module-add-identities
6deb95c4af14690b1994696efb135838b7f9b991  2026-11-21     1  b3/compact-display-readback-3d345bf
9009d99b2bd7c0ec73042422b1d87df6e39c5112  2026-11-21     2  b3/compact-display-readback-f77f8ae
b39c5623931c8b7c1dbb11e49a5f8693ee1a58b3  2026-11-21     1  b3/compact-home-pin-evidence-04a2954
625a469d1c6ef9bd80fe24bda734a7bc33b27e8f  2026-11-22     2  b3/elitebook-840-g7-trial-plan
a5dafb6fdd3d32e5214d56e1f50e6d52e8616542  2026-11-21     1  b3/enrolled-ovmf-observation-c99e428
12b06e8d9e07bb162483a7bd0769420096dbe26a  2026-11-22     1  b3/firmware-evidence-397-8eaba04c
fa1788dacabea16570e61bb4749341b5bb6827f9  2026-11-22     1  b3/firmware-evidence-401-5b683bad
4881fb057d6cfc911c20b0cd9c1714c44b008d46  2026-11-22     1  b3/firmware-evidence-401-68115b6b
0cef15740a18836c0fa31a41bbcb4358a9e972ab  2026-11-22     1  b3/firmware-evidence-401-c15e49b7
7e7c677f4c6275127d2283d5b8fa575476f8f7e6  2026-11-22     3  b3/give-up-34-fixed-observations
886f11d730807807633194cce95597a57a7a7210  2026-11-22     1  b3/give-up-405-34-observations
09323b785732ca71f4144dc9fefcf4ef7f218ca0  2026-11-22     1  b3/model-memory-ovmf-evidence
f7fd22bc7d0751c06fa128f116e14a61cc6069cb  2026-11-22     2  b3/native-forward-evidence
c28b423f7acb75ea5065257929eabf97b00c14a5  2026-11-22     3  b3/native-tokenizer-evidence
a479dd288cd6877fee1a24e6a9de3294a62d4b25  2026-11-22    13  b3/preloaded-efi-evidence
4eafe96619465f79bd3ffb466018dec6ec718673  2026-11-21    71  b3/rendezvous-evidence-20261121
d5b148c00433829a29fe1ea92a3b54ec3569729c  2026-11-22     1  b3/tailorbird33-8b-inputs
98b106b0ffb9ae445779e03f32b12433cf4447d4  2026-11-22     1  b3/tailorbird33-native-qualification
b27c57799aad38413b01290ec95862187b5751ee  2026-11-21     1  b3/uefi-cold-boot-evidence-c99e428
0fafdf4b8e56f519088057bf977f41329ff9052f  2026-11-21     6  b3/uefi-display-shadow-check
184e1b58f5d088b840091aed46980f0f3a8b09f7  2026-11-21     1  b3/uefi-original-home-evidence-751f40c
bb7732a3ab6dc5700e7d7db2b2df57eeec02cd30  2026-11-22     1  c3/model-media-diagnostic
771270eb678f329023b055840b164efaf236fab1  2026-11-22     5  c3/qwen3-8b-manifest-evidence
92cd87bd7febb42a572f39e33c848d21f0deed7b  2026-11-14     2  codex/bind-shell-observation-register
8e5fa1cc286106645088b1ccf10aff7ac023f58b  2026-11-14     1  codex/check-locked-instrument-state
53ca4e6cb396ea7709f1dea34a261b7f5c50f102  2026-11-13     2  codex/compact-machine-replies
5d7b3a9f26c63f8347099934552d8457845e1981  2026-11-14     1  codex/crypto-conformance
f79a9d11f6ede661bc4c04434046a8bc69060d09  2026-11-14     1  codex/identify-previewed-instrument-input
cc26154665a42ff85e6382406943fab4819045f8  2026-11-14     1  codex/mark-instrument-result-previews
05fb3724ef9bd6b9ede0b67f5997645e9cb41797  2026-11-14     3  codex/prepared-offer-commit
d414ac79f79f2d2db11c99da941cc5a35f103df6  2026-11-14     3  codex/preserve-repair-fragments
d79cd825d9eed5dc11343114ebc352e7946ae7b3  2026-11-14     1  codex/preserve-reported-observation
6f58b7f720994f659ea66950b804305d1ec584e2  2026-11-14     1  codex/preserve-shell-hook-id
2b70c2edfab7a2767fd69d5e77ce69f7512fbb63  2026-11-13     5  codex/r1-prefix-memory
b55fbc0a6f3172bedf56f4badcf387229581b95b  2026-11-13     2  codex/r1-timestamp-fidelity
2b0bd985660c3e2d1e9a9a0b0901d8da91d1abc9  2026-11-14     1  codex/recheck-queued-register-lock
ad80622b8d3a7afac192900fad46bfc20123e3ad  2026-11-14     1  codex/retain-live-task-authority
50c03de418c146cec7ba5f1b9738e25ad81f4b01  2026-11-14     2  codex/validate-instrument-arguments
339a2f596c04351f91def58b82d45c34a6b5d329  2026-11-14     5  codex/validate-mcp-request-envelope
c974dfce93a9bfdbadcbfe6e211291c2e2bbaa4b  2026-11-14    10  codex/verified-status-header-updates
9be6b2687737f12a9606d10529d37484c690787b  2026-11-20   159  d2-native-full-launcher-return-oracle-20261120-1832
da4a1f1932a287488d183d90c19c3fb40166fde2  2026-11-20   157  d2-native-offline-refusal-notice-20261120
fe8b67af732a0f920f0f76eead0e16b6bced0147  2026-11-20   160  d2-native-status-row-pixel-gate-20261120
ce13d4f802b02600d796a536a1ab090e5e9fbf5d  2026-11-16     1  director-1/core-residency-20261116
2d3fa53ab614dc6536d8d32d4ac04dc7f88e7a8e  2026-11-16     1  director-1/nov16-1630-residency
b1972912d170575da101b8acd21dbcfec43b8120  2026-11-16     1  director-1/rendezvous-resident-20261116
3d9d91c503459a16f74c618eaac559f1ffa323e0  2026-11-16     1  director-1/resident-qualification-20261116
02d8e5db67e2ac6c379fd3e7d34400f07d63c6d8  2026-11-16     6  director-10/blocked-read-qa
8e159ce5b066b07659de7e791bbe63c86a3d5fda  2026-11-16     3  director-10/public-worker-recovery-01
f1b7430a9c1aaabc5587f19b16e0c1e76646ef81  2026-11-16     8  director-10/register-diagnostic-20261116
74c4d4a9934ad68e1b5be380971bf3377a464c6d  2026-11-16     1  director-10/register-head-visible-20261116
d9d3823c9ba82b9158ea7b7752603ded5d75fbaf  2026-11-16     1  director-10/register-readback
ff9dfb80189ff5f4a2e291b5ff3cfb05af9bf5bc  2026-11-16     1  director-10/register-verification-display
c456f33bfdfbf08ad1aed424026f6c1c86645f33  2026-11-16     1  director-10/round-45-evidence
7c9ecef867f2182f489a850cd4a85de78ffe5936  2026-11-16     1  director-10/sleep-optical-qa
dd30eb7c75a3213e5fc6cb851a122a666c309bfc  2026-11-16     3  director-10/sleep-stop-qa
b0a21f834b0ddad7746ab7d69d0162e734c027ca  2026-11-16     3  director-11/gpt-pmbr-boot-acceptance-20261116
f6d56613f38552fe5b2236893ed97e5396080475  2026-11-16    40  director-11/register-default-build-parity-20261116
a2b38bc23ad4f28f8b90e7f9b4960ad2fec15387  2026-11-16    16  director-11/register-ram-candidate-oracle-20261116
3ee92b480953d28a851523719d15acc37f19389e  2026-11-16    17  director-12/isolated-container-boundary-probe-20261116
c24482c58fc71538cec541681a90f1b4184cb2fd  2026-11-16     8  director-12/kernel-in-oci-diagnostic-20261116
1baf4a2fe6b4d0c6ddb4b9d0d7c6163bef8bfe15  2026-11-16    47  director-12/oci-guest-restart-journal-20261116
c8b8eac9b558af73d8723f0c104cfed9c7e05550  2026-11-16     8  director-12/parrots-supervisor-loss-characterization
7e1b8e50a0206cf388cc274d6e692c816e40dcc0  2026-11-16     4  director-13/anti-brick-selector-audit-20261116
08a3d8f472c204ec422e3802fa815b37fd2abcea  2026-11-16    19  director-13/isolated-descendant-qualification-20261116
2058c5cddd9fa560a91b4fb568e8cd42de7b311b  2026-11-16    30  director-13/network-lifecycle-audit-20261116
900d5493ea067cce559241d3f74903c90a6e3087  2026-11-16     8  director-14/optional-dual-qwen-inventory-oracle-20261116
e45c42e784e18dc6f80c33919de3cc8e7d5bbe46  2026-11-16     3  director-2/display-unrecognized-output-20261116
91fde8d576649ce146ab380d0f6e74b900dce94d  2026-11-16     3  director-2/launcher-visible-executable
e002652fcd62c1378e43518d45b94248a1b4a34f  2026-11-16   103  director-2/legible-enabled-stop
651e1d2088c436283e23ce6f172497fabbdeec16  2026-11-20   152  director-2/native-editor-byte-case-oracle-20261120
8d1c965061486ffd460ffc875af5379188b3275f  2026-11-20   154  director-2/native-empty-output-guidance-oracle-20261120
66ae1206f826f233b31bd17abd6bc60a7a1c88c9  2026-11-20   150  director-2/native-f6-badge-oracle-20261120
cbadd3f606b1b5bc6ff14fefcb2e05375139c05b  2026-11-20   156  director-2/native-mode-request-affordance-oracle-20261120
16639cff26b264beacfad4f7e9120b1b666b30f6  2026-11-20   149  director-2/native-mode-rgb-acceptance-20261120
62837d8a73a5d2b25f3cd62542d8ac262b5257ae  2026-11-20   151  director-2/native-offline-status-rgb-oracle-20261120
073a9d6e959ecdad540e4dfbe3dffd861cfb0055  2026-11-20   153  director-2/native-shortcut-legend-rgb-oracle-20261120
2daf210e81b8f99c61ad362617d7e1368f9f3121  2026-11-20   155  director-2/native-task-section-label-rgb-oracle-20261120
edf83a2115577f1bc11fc5da9e8b7d5fa210d2fa  2026-11-17    11  director-2/register-640-native-negative-20261117
12de565d3c56baffd609f6c1957b69b150867072  2026-11-16     7  director-2/register-head-native-pixel-acceptance
1c3828ba9e57e56990128c8e347be29ab2458bb5  2026-11-16     4  director-2/register-native-qemu-witness-20261116
e837b5bf766a52ace30e8cb395ff874ed955961d  2026-11-16     8  director-2/register-native-qmp-witness-20261116
577f204156cbb7feddb25d8269a9ef9c0fc56c95  2026-11-17     9  director-2/register-scroll-saturation-20261117
f0474f85b1335c0b06bab20d8f385b8363e6f3db  2026-11-16     2  director-2/restore-genuine-installed-output-20261116
8ff7b8e4699cc15ac280460ba5cc7e571198e4c0  2026-11-20   144  director-2/stop-caption-native-oracle-20261120
3cfeb1964c4ffa7f7710868152b92825ee74b4d2  2026-11-20   147  director-2/stop-receipt-execution-counts-20261120
0a1d7dca739f32102a52a87eec1279d48a3627a5  2026-11-16   106  director-2/tailorbird-stop-contrast
575f2c14a6a22e5c320c4a25dcb9275f4ea441e9  2026-11-16     1  director-3/core-residency
98b66ae0413acc5477124243c7c2d58191c30005  2026-11-16     2  director-3/core-resident-1657
b1ee309bdefaf1da21129d7ee81c739984bdd8eb  2026-11-16     1  director-3/core-successor-1603
ca178d02a848ca0a8fb64ef3b54fb64a62c4875b  2026-11-16     1  director-3/native-input-device-names
58f5f4ab8fdc2800a41bbb102facf0c6500cbb72  2026-11-16     1  director-3/pending-edit-restart
934d0ddb585fce097eedb3e93a1bf8091e3866f1  2026-11-16     2  director-3/qualification-20261116
ba9374011cc11cffb466b4dabdb2285ca36a4f95  2026-11-16     4  director-3/tailorbird-deny
11e9ec776e2bac221acdf35f5e817fa13fcec9c1  2026-11-16     4  director-3/tailorbird-screen-output
09c5a2ea09ddd3383db673357baa6d507ca9ae29  2026-11-16     1  director-3/tailorbird-startup-readback
2c9039e593771f931ab0a5196ae2d775e8fad15d  2026-11-16     6  director-3/tailorbird-stop-regression
90f12c532080af6d6f5d7483a59fcb548039dbb4  2026-11-16     1  director-4/core-cohort-1640
054dec64b568e4a9ed2d6804a64443e608c9f006  2026-11-16     1  director-4/d5-g1-participant
680de3cec4f6e362a7a90dd1727bccf2f8177596  2026-11-16     1  director-4/resident-core-recovery-1534
9415936b57a0d87bfd0a0c00e078861b82f60195  2026-11-16     1  director-4/session-core-nov16
aa25262d32c96f7d8ab572da83d543286fa14866  2026-11-16     1  director-5/session-written-cohort
84e6712d17962646aba2b5b195aafc3c1d66b59b  2026-11-16    97  director-6/framebuffer-page-check
160ff7f4ec60584701cc28675e4f4fe71bef3b12  2026-11-16     1  director-6/session-core-nov16
8a3933687f88900f7589e7d1145af750f600b9fa  2026-11-16    87  director-6/tailorbird-display
eea29f09b40944841e74b443ce3ae9bd3b1fcb98  2026-11-16    86  director-6/tailorbird-output-case
2b4401769efecd2cf878ac4c1967779528ac15ec  2026-11-16   101  director-6/tailorbird-view-scale
abf6361acaf7d6910f955ef4c6de8bc596b002ad  2026-11-16     1  director-7/resident-session-1610
8f57699345a94148b30abb52f944dec4bd3d8784  2026-11-16     1  director-8/2026-11-16-resident-readback
69e727aaed33e838616b40a05fae2a45d01e8a0e  2026-11-16     1  director-8/resident-core-recovery
26cab27daca8c5bd7d453cea4692ab8e8f36cc42  2026-11-16     1  director-8/resident-core-successor
f605b244b6e73c38892975b2e78fa3756fa6b509  2026-11-16     2  director-8/surfbird-navigation-check
aaadc4741738814659511106bd7cf950f3efffd8  2026-11-16     2  director-9/boot-manifest-checkpoint
536926804fe93e1e876b494dde8224f3f53acbcb  2026-11-16     3  director-9/diagnostic-only-token-reference
f9a44c1401a3ace80259d86835e315b858a25796  2026-11-16     2  director-9/model-receipt-source-id
107206a44d7f8f92a412a31bf54117e9d5bffc53  2026-11-16     2  director-9/replay-core-nov16
d882bc9723fcebc656804f7cf68986bb36eb43bf  2026-11-16     2  director-9/resident-core-continuation
41ec63897b97c0c1709e45b9796fb7f4ba443f3d  2026-11-16     1  director-9/resident-core-nov16
af840760af1a0a112cbedce8aaa1547eeefa197d  2026-11-15     2  director-g/android-helpdesk
87f8ed401cd6df415ee5c42c3c5198b198f9acaa  2026-11-13     4  director-g/bind-local-module-removal
1034b9dc8e8169ce9a67690b6f65d2bc4ba1a659  2026-11-13     4  director-g/bounded-stop-transcript
d68229191354bfd3467b420cdb8db8006f3f5334  2026-11-14     1  director-g/cipher-trials
30cba3d45b7f3b94556f306a3304dff99d220faf  2026-11-14     1  director-g/cryptography-predicate-controls
c69e932a02e2e34e02fd71e8cd04304e18755b05  2026-11-14     3  director-g/doom-store-recovery
dd0854bd430d0ce474e95ec9aaec432173dbdd83  2026-11-14     2  director-g/durable-guest-store
525e7a877208a26a72c4b413f19fdbd7ef1ceb6f  2026-11-14     1  director-g/guest-udp-policy
36a63b1c5d496adffce66857dfed25fcc569e592  2026-11-14     1  director-g/lean-raw-string-boundary
e8d62e4b00b7bcb404b67fbba9151c50814f1d70  2026-11-14     1  director-g/science-math-svg
ab7e8ce3de88c8485e18da27a4e6d10dd4c5b149  2026-11-13     2  director-g/test-kernel-group-rollback
83bed4a75f6ac63e5578b89f8df633ba199f321b  2026-11-13     4  director-g/validate-32562dc-removal
20b9a42911cc370d946398c8ca13bb566410abf1  2026-11-13     3  director-g/validate-attempt-f69f988
de6eb231cb7682e166419405b9eff2d37ac0cc69  2026-11-13     1  director-g/validate-group-repair
81c62b920dfb4c809ad84515d422e4e7c2d72652  2026-11-13     1  director-g/validate-kernel-1a7fdd6
8fe8342f6dba8c774789cb482e7560ea32a09613  2026-11-13     1  director-g/validate-math-6abd1e8
900e3092a163768710eafd0c5ee767a227663584  2026-11-13     6  director-g/validate-transcript-fifo
e8e999b75d4f0f7d06f8f5aa5ba4e0d9f464547c  2026-11-15     4  director-pt/offline-qwen-media-checks-20261115
dec4568a08f222697167e47afd89137d7e517cdf  2026-11-15     1  docs/native-64-transition
9bb959e039ead3ff0cf80c83d79c479a9062edd8  2026-11-21     1  evidence/a3-312-0770222
8c99654a700e35b1bd34621166efa4fa4e0aceaa  2026-11-21     2  evidence/a3-312-1591757
a7c70f9aa37c5e830ef9b6d92d5a3171468538d1  2026-11-21     1  evidence/a3-312-8642b23
97e1108f35c315712bf330e1405a8bd51133c96b  2026-11-21     2  evidence/a3-314-8979270
eda9428d3154d6ea79bb6c084a7f15da7c7007d1  2026-11-21     1  evidence/a3-314-95508fa
47086f526ff3e749bfc8e90442ac33128e502016  2026-11-21     2  evidence/a3-317-30e19ed
1d3fb18be1427adfdcc2147b364c9c21c9a01d09  2026-11-21     2  evidence/a3-318-81b2674
e67f4b9d57d64c84560ff08fc01b07f313d29997  2026-11-21     1  evidence/a3-318-9da8173
84e9927a9543ec75342182ff44745b8ac8e5b529  2026-11-21     1  evidence/a3-319-27b181e
5310c3bc0eb2bedaaa3bdc4273ff1841ddab8a80  2026-11-21     1  evidence/a3-desktop-62c24cc
1757b8d5078a3c97121bfb154ddadc7da9ba9d17  2026-11-21     1  evidence/a3-f5-2c27f91
2d6f0f904c1d641a57fa225219ef438102417157  2026-11-21     1  evidence/a3-gop-1366-711a585
bc67979d56c72b69e2af117237517a78408ff988  2026-11-21     1  evidence/a3-laptop-528fbf9
afefd56eb71d0be92eb10e3287e6dfa72c490cf7  2026-11-21     1  evidence/a3-live-register-16b42f4
2234f3282c0d0cdfe2481351ebee21d41123bdb0  2026-11-21     1  evidence/a3-power-19496d2
e494cc2ecab549fc6fa15037bf8c959ef2ee86ab  2026-11-21     2  evidence/a3-readback-312-3d345bf
3d18bd8b8b1d18dd0782727223924605417eed12  2026-11-21     1  evidence/a3-readback-312-f77f8ae
7ff26327065f1019dfd7b7daf47b305117107139  2026-11-21     2  evidence/a3-readback-312-f9743bf
d0d1e99b914b97531819bd47b83f8f7f8078c6f6  2026-11-21     1  evidence/a3-small-desktop-20ff668b
c9d865f41dd5727b047eaf7f5734d96026f5edaa  2026-11-21     1  evidence/a3-small-text-0dd8ec9
0ad98d3b7ec6dce97df70d924ad572dfde2b182e  2026-11-21    10  evidence/a3-tailorbird-optical
dc713d3b8fc8fe0f07053e78af4dbe4c0796f4ea  2026-11-21     1  evidence/a3-tester-16b42f4
69296ce159b1a3c852af524dddb7e511ef597352  2026-11-21     2  evidence/a3-usb-316-926c888
2d36558d0c7674d5367b6c8e3797b219131b0625  2026-11-21     2  evidence/a3-usb-readback-751f40c
45b9d39272062bced9746a3fe400f75fb58e041a  2026-11-16     1  evidence/director-3-core-residency
179c7f455a82f3508232b555fd7f74689083d83a  2026-11-16     3  evidence/director-3-installed-pending-edit-20261116
ff0fcc92927d080a716367b2d03ff5289df3c90c  2026-11-16     1  evidence/director-3-pending-edit-restart
99ddaf3605165fcfef20a2c2e7eb9792d7fcc6c7  2026-11-16     1  evidence/director-3-resident-rounds-20261116
e6893646a2067cde24f9a65ae577a96611ff1177  2026-11-16     5  evidence/director-3-tailorbird-denial-20261116
7183fcc2401198ceac6017ea71c57869e3b52da6  2026-11-16     5  evidence/director-3-tailorbird-output-20261116
f1876b8ee8067ab79ba90f848376b6647bd9813e  2026-11-21     1  evidence/tester-cd-296-source
182d8de5e2afa6e2483375998ce3f9eca6ec9732  2026-11-14    23  feat/bound-com2-reports
d0da1cc394367757b4513c13a874caaa6a51b5b1  2026-11-14     2  fix/application-launch-observations
f4db23ec54c4d06b0abafc77fc3603aa8c572a11  2026-11-13     7  fix/build22-strict-method-tree-verification
4765e599a3b63154279dbdec7b71968426b51c99  2026-11-14     4  games/bellweather-lantern-watch
104cb29b49933882d57d991c5b77ff5986e495b8  2026-11-13    47  integration/build22-module-identity
3c0e596ec4d5ade272d2910d89435109b66d6c6a  2026-11-13     8  integration/compact-machine-interfaces
19d9b32c532095384411176b33a28f8e31b80254  2026-11-13    12  kernel/boot-report-encoding
08a1c4a7906d53044352a7b97579fbf231f95b12  2026-11-13     2  kernel/boot-report-receiver
aa0fb7ee0754f3ff3e72f9032509838c82b43bad  2026-11-13     1  kernel/compact-boot-report
adf37b86cb6a1efae0e78b9aeb23a520612fbab0  2026-11-13    10  kernel/live-boot-admission
58b1a59b0cab44221bce080169bb42ba07840e2c  2026-11-14     1  pcp/director-z-pkg-core
976e5b1abe148e24037c979ea416887ed2ce4f54  2026-11-13     2  perf/boot-report-jsonl
c925ce1ea9bb3a3a4a99ecc5fbf2f83f5e59d9a4  2026-11-14    21  perf/compact-machine-boot-output
5a20d6f4c634b793fee0d86ef87b6c38149497dc  2026-11-13     1  perf/model-context-overhead
7e6442c024861823c11edbd6220eaeee760b3138  2026-11-13     3  perf/r1-incremental-transfer
04ef4f8eed7ff365c62809dab62d138e04cecb35  2026-11-13     2  perf/single-transition
54b5903575a85d80e188258c64dc414d37be5660  2026-11-13     9  perf/verified-status-polls
374f2747a3943a60b91808bd7c7ced006bcd84fd  2026-11-19   139  session-a/pointer-f6-view-epoch-20261119
37011921ddd670ba12ef01b3319daf47344f0ad0  2026-11-20   141  session-a/pointer-mode-commit-generation-20261120
720f2bf17a4cfb79846dfe6f4667e9821b94eb90  2026-11-19   136  session-a/pointer-nonprimary-first-chord-20261119
d6094d005541041dab4fb9ed6c5276ab3f7136db  2026-11-17   130  session-a/preserve-unsent-drafts-20261117
57ff22865a4fc8458f644e916ab72a01f073e2e0  2026-11-15     7  session-a/raster-desktop-first-launch-20261115
f1b96ddd9f39b77fb638f6353a83e75c98ce3d99  2026-11-15    11  session-a/raster-desktop-native-20261115
81c4469c521aa73d15d3cbbe8a02cfdb7edcf127  2026-11-19   135  session-a/retain-unsent-drafts-over-feed-20261119
1b7970e36c9705e106ec780d195497b4e5200071  2026-11-20   141  session-a/tailorbird-800-title-oracle-20261120
342e55c5dc248c8bc5111e89dddd7e7eb552517a  2026-11-17   134  session-a/tailorbird-feed-window-20261117
3c762e6111369bce752a3d0c8b4d0d2a93850aa3  2026-11-16   129  session-a/tailorbird-native-ui-20261115
a03f123b9a25519965d996e6489d4494de4b59c7  2026-11-17   131  session-a/tailorbird-output-byte-count-20261117
bdb5b6e87d9762415f038158870ca9c8f365028b  2026-11-20   146  session-a/tailorbird-python-assert-guard-20261120
bc8d3d229df56eec9acb39410c8ee2edae9a08ce  2026-11-20   148  session-a/tailorbird-stop-heading-oracle-20261120
11637efe8c5dcd9b395efedc581cd848dade58ae  2026-11-19   140  session-a/tailorbird-verified-elf-handoff-20261119
9e79ee041cba823bc1445407980e2dff67e194b6  2026-11-15     6  session-b/boot-image-structural-audit-20261115
922c440c45b979ade30aef10380e8ee8d2ba2395  2026-11-16     6  session-b/gpt-recognition-cross-oracle-20261116
fe94ea85add17f826476aabe0639ec41496b9ca5  2026-11-15     5  session-b/ide-role-boundary-test-20261115
0e648ca36ef19aa53c5025100ae026a5695fb6de  2026-11-16    26  session-b/ide-volume-pin-guard-20261115
1ed2f3ce7721bb2957c0c9ce44fd203437dd91ec  2026-11-15    24  session-b/mb2-efi-info-inspector-20261115
e137e4cf14711af662fc7e3575c3a1f9809beade  2026-11-15    19  session-b/native-64-handoff-contract-20261115
db54f53c24e5639922b00c9ec44b08be138ed56f  2026-11-15    42  session-b/uefi-gop-observation-probe-20261115
bd95e39ecb8983ced21643a43fd91c33c6921315  2026-11-16    10  session-b/virtio-gpt-readonly-20261115
dbab76c65ace855b85a842e52c3ebfcc496691ac  2026-11-16    31  session-b/virtio-gpt-target-preflight-source-20261116
0c8445d3793317cfea220eb731d3dabaf9af90bf  2026-11-14     2  session-c/educational-spn-16bit-known-plaintext-20261114
1c862f03a70e0406e98c76e64839e02fad65e021  2026-11-22     1  z3/branches-kept
948fcf7005472f633b59e544431558c250eab5bc  2026-11-22     1  z3/catalogue-reference-links
63f87ff6d042bb44e442c8c36f882067c131a501  2026-11-22     1  z3/details-machine
b2fd260018870e7a38fe82d6bd3c9b46f1ecdcf7  2026-11-22     6  z3/hard-float-guest
801996b98aee1024ccfee901a7430ef5b759c59d  2026-11-22     2  z3/processors-threads
745e8831cb25090ce478fcf246dcf6c8a65ec7b2  2026-11-21   130  z3/tailorbird-audit
a4276d841a0179cf4855ffcd25da663876a0c646  2026-11-22    73  z3/task-records
```
