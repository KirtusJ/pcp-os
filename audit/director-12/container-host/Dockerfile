# Deliberately minimal test container. It contains one static Rust binary.
# The Docker host supplies process namespaces, cgroup accounting and PID 1.
FROM scratch
COPY --chmod=0555 worker /worker
USER 65534:65534
WORKDIR /
ENTRYPOINT ["/worker"]
