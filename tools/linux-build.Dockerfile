FROM rust:1-bookworm
RUN apt-get update \
    && apt-get install -y --no-install-recommends libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev build-essential \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src/src-tauri
