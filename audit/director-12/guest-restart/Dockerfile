# Linux OCI container is a host for a *QEMU guest*, not a PCP kernel host.
# All files and attempts are synthetic test-only material on a disposable runner.
FROM ubuntu:24.04
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update -qq \
    && apt-get install -y --no-install-recommends qemu-system-x86 seabios \
    && rm -rf /var/lib/apt/lists/*
COPY --chmod=0555 d12-runner /d12-runner
USER 65534:65534
ENTRYPOINT ["/d12-runner"]
