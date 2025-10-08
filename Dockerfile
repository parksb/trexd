FROM rust:1.77-slim as builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add aarch64-unknown-linux-musl x86_64-unknown-linux-musl

COPY . .

ARG TARGETARCH
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        TARGET=aarch64-unknown-linux-musl; \
    else \
        TARGET=x86_64-unknown-linux-musl; \
    fi && \
    cargo build --release --target $TARGET && \
    cp /app/target/$TARGET/release/trexd /app/trexd

FROM gcr.io/distroless/static:nonroot

COPY --from=builder /app/trexd /usr/local/bin/trexd

USER nonroot

ENTRYPOINT ["/usr/local/bin/trexd"]
CMD ["trexd", "-d"]
