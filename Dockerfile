# Production image: the axum server plus the frontend bundle it serves at `/`.
# The two build stages are independent, so BuildKit runs them in parallel.

FROM rust:1.89-bookworm AS web
RUN rustup target add wasm32-unknown-unknown \
    && cargo install dioxus-cli@0.7.10 --locked
WORKDIR /src/web
COPY web/ .
RUN dx bundle --platform web --release --out-dir /out

FROM rust:1.89-bookworm AS server
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY migrations/ migrations/
COPY words/ words/
RUN cargo build --release --locked

# Distroless has no shell or package manager. sqlx bundles its TLS root certificates, so the
# image needs nothing beyond glibc.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=server /src/target/release/platen /app/platen
COPY --from=web /out/public /app/public
ENV STATIC_DIR=/app/public \
    BIND_ADDR=0.0.0.0:3000 \
    RUST_LOG=info
EXPOSE 3000
ENTRYPOINT ["/app/platen"]
