# Reproducible Build Container for Intent-Native OS
# Pinned to Ubuntu 24.04 LTS baseline
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/opt/rustup
ENV CARGO_HOME=/opt/cargo
ENV PATH="/opt/cargo/bin:${PATH}"

# Install base toolchains: OCaml, Dune, QEMU, build essentials (No Python)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    git \
    ocaml-nox \
    ocaml-compiler-libs \
    dune \
    libalcotest-ocaml-dev \
    qemu-system-x86 \
    ovmf \
    nasm \
    mtools \
    dosfstools \
    ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Install Rust toolchain with bare-metal targets
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly \
 && rustup target add x86_64-unknown-none x86_64-unknown-uefi \
 && rustup component add rust-src llvm-tools-preview

WORKDIR /workspace
COPY . /workspace

CMD ["dune", "build"]
