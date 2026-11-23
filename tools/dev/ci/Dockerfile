# act supplies Actions plumbing; install this repository's Linux build dependencies.
FROM catthehacker/ubuntu:act-24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential cmake clang libclang-dev pkg-config libssl-dev curl ca-certificates \
    python3 python3-venv libgtk-3-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev \
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    && curl -fsSL https://packages.microsoft.com/config/ubuntu/24.04/packages-microsoft-prod.deb -o /tmp/microsoft.deb \
    && dpkg -i /tmp/microsoft.deb && apt-get update \
    && apt-get install -y --no-install-recommends powershell \
    && rm -f /tmp/microsoft.deb && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
ENV PATH="/opt/cargo/bin:${PATH}"
RUN curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o /tmp/rustup.sh \
    && sh /tmp/rustup.sh -y --profile minimal --default-toolchain stable --component rustfmt,clippy \
    && rm /tmp/rustup.sh \
    && cargo --version && pwsh --version && python3 --version
