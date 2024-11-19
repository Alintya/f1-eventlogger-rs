FROM rust:latest

# Update package lists and install required packages
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    gcc-mingw-w64 \
    g++-mingw-w64 \
    zsh \
    sudo \
    && rm -rf /var/lib/apt/lists/*

# Create vscode user and add to sudo group
RUN useradd -m vscode && \
    usermod -aG sudo vscode && \
    echo "vscode ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers && \
    chsh -s $(which zsh) vscode

# Install Oh My Zsh for vscode user
# Switch to vscode user and set up environment
USER vscode
WORKDIR /home/vscode
RUN sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)"

# Install nightly toolchain
RUN rustup toolchain install nightly && \
    rustup default nightly

# Add targets for nightly
RUN rustup target add --toolchain nightly \
    x86_64-unknown-linux-gnu \
    x86_64-pc-windows-gnu

# Install components
RUN rustup component add clippy

# Verify installations
RUN rustc --version && \
    cargo --version && \
    rustup show && \
    rustup target list --installed