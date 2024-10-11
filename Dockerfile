FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app
LABEL org.opencontainers.image.source=https://github.com/paradigmxyz/reth
LABEL org.opencontainers.image.licenses="MIT OR Apache-2.0"

# Install system dependencies
RUN apt-get update && apt-get -y upgrade && apt-get install -y libclang-dev pkg-config git

# Builds a cargo-chef plan
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build profile, release by default
ARG BUILD_PROFILE=release
ENV BUILD_PROFILE $BUILD_PROFILE

# Extra Cargo flags
ARG RUSTFLAGS=""
ENV RUSTFLAGS "$RUSTFLAGS"

# Extra Cargo features
ARG FEATURES=""
ENV FEATURES $FEATURES

# Builds dependencies
RUN cargo chef cook --profile $BUILD_PROFILE --features "$FEATURES" --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --profile $BUILD_PROFILE --features "$FEATURES" --locked --bin reth

# Hack: Add a cache busting step (above steps are the more
# time consuming ones but we need to make sure the rbuilder is 
# always freshly cloned and not cached !)
# Since the content of this file will change 
# with each build, Docker will consider this 
# layer (and all subsequent layers) as modified,
# forcing a re-execution of the following steps.
# ADD https://worldtimeapi.org/api/ip /tmp/bustcache

# Clone and build rbuilder (gwyneth branch)
RUN git clone -b gwyneth https://github.com/taikoxyz/rbuilder.git /app/rbuilder
WORKDIR /app/rbuilder
RUN cargo build --release

# Copy binaries to a temporary location
RUN cp /app/target/$BUILD_PROFILE/reth /app/reth
RUN cp /app/rbuilder/target/release/rbuilder /app/rbuilder

# Use Ubuntu as the release image
FROM ubuntu:22.04 AS runtime
WORKDIR /app

# Install necessary runtime dependencies and Rust/Cargo
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Install Rust and Cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Copy reth and rbuilder binaries over from the build stage
COPY --from=builder /app/reth /usr/local/bin
COPY --from=builder /app/rbuilder /usr/local/bin

# Copy the entire rbuilder repository
COPY --from=builder /app/rbuilder /app/rbuilder

# Copy licenses
COPY LICENSE-* ./

# Create start script
RUN echo '#!/bin/bash\nrbuilder run /app/rbuilder/config-gwyneth-reth.toml' > /app/start_rbuilder.sh && \
    chmod +x /app/start_rbuilder.sh

EXPOSE 30303 30303/udp 9001 8545 8546
ENTRYPOINT ["/usr/local/bin/reth"]
