FROM node:24-bookworm AS node

FROM ubuntu:24.04 AS build

ARG RUST_VERSION=stable
ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:${PATH}"

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    dpkg-dev \
    file \
    libayatana-appindicator3-dev \
    libdbus-1-dev \
    libgtk-3-dev \
    librsvg2-dev \
    libssl-dev \
    libwebkit2gtk-4.1-dev \
    libxdo-dev \
    pkg-config \
    wget \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s /usr/local/lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -s /usr/local/lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain "${RUST_VERSION}"

WORKDIR /app

COPY package.json package-lock.json ./
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock ./src-tauri/
RUN npm ci \
    && cargo fetch --locked --manifest-path src-tauri/Cargo.toml

COPY . .
RUN npm run tauri build -- --bundles deb

FROM scratch AS artifacts
COPY --from=build /app/src-tauri/target/release/bundle/deb/*.deb /artifacts/
