# Disposable Linux container for QEMU, NOT a PCP OS bare-metal container or product image.
# Base image is identified at runtime by its OCI digest; ubuntu:24.04 is not
# content-pinned here. Record provenance before drawing release conclusions.
FROM ubuntu:24.04
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update -qq \
    && apt-get install -y --no-install-recommends qemu-system-x86 seabios \
    && rm -rf /var/lib/apt/lists/*
USER 65534:65534
ENTRYPOINT ["/usr/bin/qemu-system-x86_64"]
