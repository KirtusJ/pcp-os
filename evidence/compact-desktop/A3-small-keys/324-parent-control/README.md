# Frozen desktop checker against the preceding desktop

I ran the K1 comparisons in the unchanged settings_trial and list_trial functions and the first K2 screen acceptance predicate from frozen #324 against saved QEMU scanouts. The baseline guest was 62c24cc2, whose complete tree equals master c2cb6563. Each letter had been sent independently, with its original surface restored afterward. This is an offline comparison using original guest frames. It measures the settled pixels.

At 320×200 on the preceding desktop, five of six bare letters changed Home and five changed Programs. The K2 title predicate rejected the actual Ctrl+D frame. The older list also failed P3 for both selected filenames because its footer used the old shortcuts. The frozen tree's payload produced no K1 changes. Its stock list_trial passed every predicate including all 12 Details pages. The newer original frames came from the measured kernel and Surfbird payloads that were identical between 3920027 and 17f6b30. The checker used for both sides is blob f319d433d4d79dca9e7bb3d62d1df7e5e65bfde5 from 17f6b30.

The Home replay ends immediately after the six letter comparisons, before Settings opens. Programs runs to the first rejected frame or to the end of the stock function. The two loader programs on the preceding desktop were identical. The negative K2 result concerns its actual title, before the checker reads any file digests. The newer fixture has two distinct programs.

Extract originals.tar.xz, verify SHA256SUMS, and run python -IB check.py /path/to/17f6b30 parent current/320x200 inside the extracted directory. result.json records every invoked frame and the original checker messages.
