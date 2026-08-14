# syntax=docker/dockerfile:1.7@sha256:a57df69d0ea827fb7266491f2813635de6f17269be881f696fbfdf2d83dda33e

ARG RUST_IMAGE=rust:1.85.0-bookworm@sha256:0ff31c9ffa641a62e48d543fb00b4960955ea375f40776f40f585b89e654cc5e
ARG NODE_IMAGE=node:22.23.2-bookworm-slim@sha256:d649c27dae7ba0137b3cef5dd75baa422c08dc3d9e3fc0c23dfb172dc3cc6436
ARG CHROME_ARCHIVE_SOURCE=chrome-download-remote

FROM ${RUST_IMAGE} AS rust-builder
ENV CARGO_HTTP_TIMEOUT=600 \
    CARGO_HTTP_LOW_SPEED_LIMIT=1 \
    CARGO_NET_RETRY=10
WORKDIR /source
COPY Cargo.toml Cargo.lock ./
COPY compat ./compat
COPY crates ./crates
RUN --mount=type=cache,id=a3s-test-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=a3s-test-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=a3s-test-cargo-target,target=/source/target \
    cargo build --locked --release --package a3s-test-cli \
    && install -D -m 0555 target/release/a3s-test /output/a3s-test \
    && strip /output/a3s-test

FROM ${NODE_IMAGE} AS browser-cli
WORKDIR /opt/a3s-test-runner
COPY images/runner/package.json images/runner/package-lock.json ./
RUN --mount=type=cache,id=a3s-test-npm-cache,target=/root/.npm \
    npm ci --omit=dev --no-audit --no-fund \
    && ./node_modules/.bin/agent-browser --version \
        | grep --fixed-strings --line-regexp 'agent-browser 0.26.0' \
    && install -D -m 0644 node_modules/agent-browser/package.json \
        /output/node_modules/agent-browser/package.json \
    && install -D -m 0644 node_modules/agent-browser/LICENSE \
        /output/node_modules/agent-browser/LICENSE \
    && install -D -m 0555 node_modules/agent-browser/bin/agent-browser.js \
        /output/node_modules/agent-browser/bin/agent-browser.js \
    && install -D -m 0555 node_modules/agent-browser/bin/agent-browser-linux-x64 \
        /output/node_modules/agent-browser/bin/agent-browser-linux-x64 \
    && mkdir -p /output/node_modules/.bin \
    && ln -s ../agent-browser/bin/agent-browser.js /output/node_modules/.bin/agent-browser \
    && /output/node_modules/.bin/agent-browser --version \
        | grep --fixed-strings --line-regexp 'agent-browser 0.26.0' \
    && npm cache clean --force

FROM scratch AS chrome-download-remote
ADD --checksum=sha256:b8aeb7dfc8ebb25b66fcc375c5505ea3715556f019bc4a87ffd19d7bfaa4ff51 \
    https://storage.googleapis.com/chrome-for-testing-public/147.0.7727.117/linux64/chrome-headless-shell-linux64.zip \
    /chrome-headless-shell.zip

FROM ${CHROME_ARCHIVE_SOURCE} AS chrome-download

FROM --platform=$BUILDPLATFORM ${NODE_IMAGE} AS chrome-assets
ARG DEBIAN_SNAPSHOT=20260813T202440Z
ARG DEBIAN_SECURITY_SNAPSHOT=20260813T211357Z
COPY --from=chrome-download /chrome-headless-shell.zip /tmp/chrome-headless-shell.zip
RUN printf '%s\n' \
        "deb [check-valid-until=no] http://snapshot.debian.org/archive/debian/${DEBIAN_SNAPSHOT}/ bookworm main" \
        "deb [check-valid-until=no] http://snapshot.debian.org/archive/debian-security/${DEBIAN_SECURITY_SNAPSHOT}/ bookworm-security main" \
        > /etc/apt/sources.list \
    && rm -f /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install --yes --no-install-recommends \
        unzip \
    && unzip -q /tmp/chrome-headless-shell.zip -d /opt \
    && mv /opt/chrome-headless-shell-linux64 /opt/chrome-headless-shell \
    && test -x /opt/chrome-headless-shell/chrome-headless-shell

FROM ${NODE_IMAGE}
ARG DEBIAN_SNAPSHOT=20260813T202440Z
ARG DEBIAN_SECURITY_SNAPSHOT=20260813T211357Z
ARG CHROME_HEADLESS_SHELL_VERSION=147.0.7727.117
ARG IMAGE_VERSION=dev
ARG SOURCE_REVISION=unknown
LABEL org.opencontainers.image.source="https://github.com/A3S-Lab/Test" \
      org.opencontainers.image.description="Hermetic Linux/amd64 Web and TUI runner for A3S Test" \
      org.opencontainers.image.licenses="MIT" \
      io.a3s.test.runner.browser="agent-browser@0.26.0" \
      io.a3s.test.runner.chrome-headless-shell="${CHROME_HEADLESS_SHELL_VERSION}"
RUN printf '%s\n' \
        "deb [check-valid-until=no] http://snapshot.debian.org/archive/debian/${DEBIAN_SNAPSHOT}/ bookworm main" \
        "deb [check-valid-until=no] http://snapshot.debian.org/archive/debian-security/${DEBIAN_SECURITY_SNAPSHOT}/ bookworm-security main" \
        > /etc/apt/sources.list \
    && rm -f /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install --yes --no-install-recommends \
        ca-certificates \
        fonts-liberation \
        libasound2 \
        libatk-bridge2.0-0 \
        libatk1.0-0 \
        libcairo2 \
        libcups2 \
        libdbus-1-3 \
        libdrm2 \
        libexpat1 \
        libgbm1 \
        libglib2.0-0 \
        libgtk-3-0 \
        libnspr4 \
        libnss3 \
        libpango-1.0-0 \
        libx11-6 \
        libx11-xcb1 \
        libxcb1 \
        libxcomposite1 \
        libxdamage1 \
        libxext6 \
        libxfixes3 \
        libxkbcommon0 \
        libxrandr2 \
        tini \
    && rm -rf /var/lib/apt/lists/* /etc/apt/sources.list

LABEL org.opencontainers.image.version="${IMAGE_VERSION}" \
      org.opencontainers.image.revision="${SOURCE_REVISION}" \
      io.a3s.test.runner.capability-protocol="a3s.test.worker-capabilities/1" \
      io.a3s.test.runner.remote-worker-protocol="a3s.test.remote-worker/1"

COPY --from=rust-builder --chmod=0555 /output/a3s-test /usr/local/bin/a3s-test
COPY --from=browser-cli /output/node_modules /opt/a3s-test-runner/node_modules
COPY --from=chrome-assets /opt/chrome-headless-shell /opt/chrome-headless-shell
RUN install -d -m 0755 /usr/share/licenses/a3s-test
COPY --chmod=0444 LICENSE /usr/share/licenses/a3s-test/LICENSE
COPY --chmod=0555 images/runner/entrypoint.sh /usr/local/bin/a3s-test-runner-entrypoint
COPY --chmod=0555 images/runner/smoke /opt/a3s-test-runner/smoke

ENV PATH="/opt/a3s-test-runner/node_modules/.bin:${PATH}" \
    AGENT_BROWSER_EXECUTABLE_PATH="/opt/chrome-headless-shell/chrome-headless-shell" \
    AGENT_BROWSER_ARGS="--no-sandbox" \
    HOME="/tmp/a3s-test-home" \
    XDG_CACHE_HOME="/tmp/a3s-test-home/cache" \
    XDG_CONFIG_HOME="/tmp/a3s-test-home/config" \
    TMPDIR="/tmp"

RUN a3s-test --version \
    && agent-browser --version \
        | grep --fixed-strings --line-regexp 'agent-browser 0.26.0' \
    && /opt/chrome-headless-shell/chrome-headless-shell --version \
        | grep --fixed-strings "${CHROME_HEADLESS_SHELL_VERSION}"

WORKDIR /workspace
USER node:node
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/a3s-test-runner-entrypoint"]
CMD ["a3s-test", "--help"]
