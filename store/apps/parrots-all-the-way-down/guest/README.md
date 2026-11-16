# Parrots all the way Down

The native guest stores its accepted queue operations in `parrots.queue`. Each successful change replaces the saved file. On a mounted durable guest store, the app also requests a directory sync. The header displays **Disk save** after a successful sync and **Session save** when the kernel provides an in-memory store.

If the saved queue cannot be read or fails validation, the guest disables all changes. Exit with Escape. Preserve a copy of the guest disk, correct read access or restore a known-good copy of `parrots.queue` in that guest store, then reopen the application. On a kernel with a durable store, sync the directory after replacing the file. An absent file starts a fresh queue.
