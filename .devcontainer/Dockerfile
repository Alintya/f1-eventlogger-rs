FROM mcr.microsoft.com/devcontainers/rust:latest

# Update package lists and install cross-compilation tools
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    gcc-mingw-w64 \
    g++-mingw-w64 \
    && rm -rf /var/lib/apt/lists/*

# Install nightly toolchain
RUN rustup toolchain install nightly && \
    rustup default nightly

# Add targets for nightly
RUN rustup target add --toolchain nightly \
    x86_64-unknown-linux-gnu \
    x86_64-pc-windows-gnu

# Verify installations
RUN rustc --version && \
    cargo --version && \
    rustup show && \
    rustup target list --installed