# Branches proposed for removal, November 2026

Part of the tidying the owner asked for (issue 368). This file is written **before** any branch is
removed, so that each can be put back. Nothing listed here holds a commit that the default branch
does not: every tip below is an ancestor of the pinned default commit named with it.

## How the lists were made, and by whom

Director B3 and Director Z3 each computed, on a complete (not shallow) history, the branches whose
tips are contained in the pinned default commit, took out the branches that are the base of an open
pull request and the branches held because they carry records, and compared the result. The two
lists were the same name for name and tip for tip:

| Repository | Pinned default | Contained | Agreed list | SHA-256 of the agreed list |
| --- | --- | ---: | ---: | --- |
| `KirtusJ/pcp-os` | `355e5c13216c715c5464d016e28cf502907d0699` | 114 | 91 | `14b397db33482efdc6546711e8087bc29338dd4758b6ed57c5f142c490bca5b0` |
| `KirtusJ/tailorbird` | `f1613bed737a00c96d0756d3b18b9c7284ad761c` | 26 | 23 | `ce3cca12e1a478edf67ca0e2f7c74643557b34eb5e528d81994e4cb9b7fe36bb` |

The digest is of the list in name order, each line a full tip, two spaces, the name, and a line feed.
B3's lists, the holds and the command are in issue 368.

An earlier count by Z3 (88 contained) was made on a shallow clone and was wrong.

## What is removed, and what is not

From the agreed lists, branches of sessions that are at work are left alone unless their owner asks:
`a3/`, `b3/`, `c3/` and Director 2's `director-2/`; and in tailorbird the branch a working session is
assigned to push to. Record holds and open pull-request bases were never in the lists. Branches with
commits outside the default branch are not touched.

To put one back: `git push origin <tip>:refs/heads/<name>`.

## pcp-os: 81 to remove

```
15a2b46059a85e738dc8026a5efd9ab69c053619  browser/surfbird-guest-http
1b55a7c9656a54503d0b52a5bc4c7ef481248f31  browser/surfbird-reader
59bfb6c37e92acadd8940a71c3a2dd67b8adfc4c  codex/open-register-once
b11f25531bff6c00135120edfe0d63fcb15dd16b  codex/refuse-mistyped-digest-claim
ef5ac03cd2d44190c10528712f2ab81cf4750ee5  codex/register-event-size-parity
858da7c1dc19b8f41558d73609fdce378b02638e  codex/replay-session-bound
59e409e16f362979645186b6ee2056677d4e0b18  codex/roundtrip-register-depth
129f73bce88f8e214bcb2c33919009b1d7ced046  codex/simulator-instrument-inventory
5330406a64128690681a7f5fe41b689e08283028  codex/verify-complete-clause-replies
02ac1aba729b4aa6bb6c2fcbb690a6311133461a  director-1/core-residency-base-20261116
e2473d01a092bfbbbf675b48311847f9cb8cbb80  director-1/installed-acceptance
71b5d3e8ce0b2ba63a1bd9237dda771c8908f45d  director-1/nov16-1630-residency-base
d236234e31c69704beb7bf0c4984488e1b8756d8  director-1/resident-qualification-base-20261116
d969d193e867c64271075f3a73689b89d95c81b1  director-10/sleep-stop-pacing
a105e22f7bc262145511817ecfe7bcfd2939ac3c  director-3/core-residency-base
fb472efc55d0736e8292363e9324f9a685df61f3  director-3/installed-tailorbird-capture-ci
7aeefbf1045a0c3b2d518f8c051e6bfe7a2383cb  director-3/qualification-20261116-base
2b084e6f37b8500514ac6962abc7eed098e15aef  director-3/tailorbird-checker-ci
2f5d8a6684d61bcd8008c6a8ec2505f3a207afff  director-3/tailorbird-receipt-lifetime
726b783864735eb806f5d7c5a96e774a6007225d  director-4/build22-retrieval-assessor
088a53a44500a921d98e995b93571ca8bd637ab0  director-4/core-cohort-base-1640
50e98308af83f62b51c700e35f07be25454351f3  director-4/core-text-inference
50e98308af83f62b51c700e35f07be25454351f3  director-4/d5-g1-assessment-base
3f5d1f0c3a7a7d220ec75c0385010c9042d13282  director-4/guest-method-readback
7f493605f8232c5fc5e6530261a81cf9c2b9295f  director-4/public-worker-core
71b5d3e8ce0b2ba63a1bd9237dda771c8908f45d  director-4/resident-core-recovery-base-1534
3c03c7cd31b90ca65f5678aff90b179a2e0b41ad  director-4/retrieval-score-reach
c6b4bf892005e4e0de1e89ab7d804604e1ddfdd5  director-4/session-core-base-nov16
f9f8a27589f3aaffb5548648b2d992d1a60cfc07  director-4/standalone-core-readback
179190e0852d0e1740d978352110cb11770638c5  director-4/test-proposed-worker-core
50e147446cd7222c1e98385501792dd1eab28507  director-5/resident-worker-supervisor
71b5d3e8ce0b2ba63a1bd9237dda771c8908f45d  director-6/session-core-base-nov16
0872076e7366a2751981af640033c63545a9d884  director-7/app-readback-names
c6ca4e6365bc5559b3c5b0ab08ca6274dcb2f78f  director-8/native-href-characters
c54e78fb1d14af8da17c764ac948998b2622f3b9  director-8/native-link-directories
a7c852f845c2f208ce9e0d86d7e00c959547291e  director-8/native-session-history
edfae31bc3f455146bf9c62683c54016f2a92947  director-8/resident-core-session-base
e4d401f2bd00c0ffdda32a99a35a1ab2adf7b0ef  director-8/surfbird-keyboard-focus
88c566128e4d31771c66309fb0d780c495a714e7  director-8/surfbird-native-links
c942213d0c5d16c7790571d97fbc8d1300c0cb93  director-8/surfbird-readable-page
93189e3e5253f9d59948ef2650f158150dbaf361  director-9/gpt-active-entry-map
21895603e53e403c296d688ab287f7a7b23de82e  director-9/gpt-mirror-validation
7329cadd9dc23e446cefae2181a6f4300c30cc19  director-9/installed-model-partition
1bf26ddd694f11ee83a7ae4654d14d9000e1022e  director-9/offline-model-files
3f8ee178750f3b10c3f150a4579e8b8c366fd559  director-9/resident-core-continuation-base
0d0007eece6d02af6119d61f83b6cc3ea01f7e3d  director-9/resident-core-nov16-base
ebbaa3a5e2fe36e38b0dcb4731bada363e3ba2b2  director-9/sparse-model-volume
1fcb69b62b028641701a5cfd8bcfefc767f4f11f  director-9/verify-handoff-digests
cae64d73eff0a1bd03eb835766be61052917763d  director-b/keyrook-recovery-hardening
b0c55f5df2ec4824bda9e88fcbd5fa1e0fa0cb64  director-b3/optical-disk-preservation
81c21b9e9a2c18583fc3769290d2c77057dca69b  director-g/async-attempt-identity
5fa8da3b58aaf93acb2c29e7239c34722ee8b0dc  director-g/incident-counts
9c6f7d45244ca54e5109e1a73b50e4bf535d707c  director-g/kernel-admission-rollback
14740c35753900f2e839db952ad0738b538414b8  director-g/keyrook-stage-name
6abd1e8247cfd5007c05f6a9176d5ae0d2288118  director-g/native-instrument-hardening
78807e7e750d77fa56f0bf32c9d19fe758a9f655  director-g/protected-bin-instrument
818e68287e7fe5f01ffeabbacff07b66fafed806  director-g/qwen-read-only-onboarding
bf5327cc5643538f4301bf63b71e97ed2de04b04  director-g/recursion-helpdesk
1322029a082eac5a3b1176f3d3dacb21bd2a13aa  director-g/rust-policy-readme
17ab663acfe97197cb1846a4035aae5b12f0aead  director-g/store-catalog
7ed093061ff245c0d914a4a52d19db26e91516fb  director-g/surfbird-integration-docs
4393e740802f14da7b3560c8b5df0b728ddfd349  director7/approved-app-readback
266ab0830d103e50323c011a20331f32954e8fd0  fix/canonical-register-vector-lf
d8c16bcfb8e196f9386627a98564c41e26194669  games/night-post-and-bellweather
c67d447721458f457395ea3d0019cf9936dad32d  kernel/isolated-register-extension
6ae0079281cc99bb099aa2658dad168df8cc577a  kernel/isolated-virtio-rx-reply
6d330a4c5c2d8e76055e1ab15266dfc231b86af0  kernel/isolated-virtio-tx-probe
734213ebbf694bd768ced6db25d5de5457b2c83e  kernel/native-guest-worker
24bd5711e188b93ec6ec65890aabf1aee3f59cd5  pcp/director-z-admission-oracle
70cadbd03c9773bf014bca60a5cfe616d6167f9f  pcp/director-z-async-note
071fe676a90f3b73e0a71f1b1b65725cf17be5e4  pcp/director-z-documents-in-step
1a7fdd69da792876376ca1809bb0f8983bcaf67b  pcp/director-z-kernel-async
f4199545b6d3fa4a01f97fa3ad660e60cece935c  pcp/director-z-mathematics-statement
caf71d98b55cec2657a55e86da73c055c84510b6  pcp/director-z-package-core
93feeb35d85895723d26618e23146dc8785802dd  pcp/director-z-package-transaction
3e7ece760725bdad898a46781e07e8677fd45adb  pcp/director-z-statement-unpinned
3204b4bb551f1fe6c899cc339309d1fdf7043468  store/gullpost-express-alpha-v1
a1436e4f3dde8b4cc271848cb5bdd915af6d39a4  store/native-heliograph-sunspur-rink
bf8c0bcdad8cf960a524c74954f143d858426ce6  surfbird/native-av-fixture
53711347ae68c51c64cd434dcf34096b4a85de17  test/guest-ethernet-admission
fe954767155ca660262466bc5d9a2137df91ac0b  z3/h9-whole-screen
```

Left in place from the agreed list (10):

```
73fdd149f975c995444a0794f0e4cf1746b5e120  b3/combined-bios-vm-tester
3290e96ff487cf7f98aa9b6cbdb85c0c8f1f8e3b  b3/consent-before-companion
d9ce2dc513994858a685287bc2f0e8bca2aab9d3  c3/efi-loader-stress
0b1eacd5160b6b4d865ec67194a644ed07067e5e  director-2/display-mode-acceptance
9c179e022cf83585e1c8f2c719166171c1c6a6da  director-2/installed-display-serial
7694d98435389e1fb34add63d28d0443849465b2  director-2/installed-title-check
d236234e31c69704beb7bf0c4984488e1b8756d8  director-2/resident-replay-base-nov16
1649526b16fbedba231eb57c1beda7c92770a10f  director-2/resident-replay-nov16
c0bf80d05564aa7712c839f893cfce6986359ae4  director-2/verify-complete-event-label
37d94dc722813b66a73a823d74b3883832991769  director-2/verify-installed-launcher-paint
```

## tailorbird: 8 to remove

```
efb45fe597eab166f2c3ef93dab0397c169bfbc7  import/post-134
f42286b9366416d6a61c7837b5175d5354c9bec1  z3/after-stack
94847b59b9209a7c4c22b0cb16329866bdb4d7d5  z3/pin-master
9efd5352c3877418765a7fc05d215bae01f44306  z3/post-134-port
e01dd406014541125d204b7744816d35d71b10bd  z3/reading-progress
28dc03b03cc89d7d6798b40744ebd0f6153ac0cd  z3/ready-notice-with-warning
1d48a345bf4062012315edb4650a6ce790438a4b  z3/taylor-projects
0fbf3826c159201bc583b9f09f8b6f8d1a1d4ac0  z3/taylor-talk
```

Left in place from the agreed list (15):

```
46ce5f070c48313696380d5c63f04d2bc332a45c  b3/repeat-note-shortcut
71c5ba6fd77994683480e614640a929391b79374  c3/avx2-worker
cd89f12f39ec80e891062f7fd8545f72105d2000  c3/demo-disclosure-check
6bcb97ee24f988a74f13551a2898aa2cb2360c55  c3/folder-snapshot
0a860f5224e702665f9e3d53a74f164fb2cfbc54  c3/live-work-ui
dd8633d8516cb1bdc0fd8f258260594ad98848f1  c3/model-worker-prefill
ffffc081dfd545fa2c65a94dc9a01cd144b14c34  c3/native-model-worker
b16c3b217f9f46b13da1fb7f344bed1e380dc5ad  c3/native-test-workflow
1c1cd3014721c0f6562f262558c1f57b413b04f5  c3/project-entry
15a0f8b1ec46e0ad3e6d3fcdc1bf4dd8dcb7be7f  c3/project-location
064b41edb7deb3a21bb98f379ae21a61c72ac640  c3/project-session
5638f4bf8e1c10b3ef9d4c022563b69e7f1664f2  c3/q8k-worker
a4f8f8a1108c6609a4d05ca1f414241f41a1ce93  c3/typed-proposal
f96a1fea783bf0d03b3c1fe5f774ebbdaf06dad5  c3/worker-reading-progress
312c6273d911887711b20d49087119444cc33df1  claude/upbeat-goodall-ssz94e
```

## State

Proposed. This file will be amended with the date of removal and the branch counts before and after
once a second director has agreed to the reduced set and the branches have been removed.
