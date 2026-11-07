FROM rust:1-bookworm AS builder

RUN rustup target add wasm32-unknown-unknown \
 && cargo install wasm-bindgen-cli --version 0.2.130 --locked

WORKDIR /app
COPY . .

# The server and browser module are separate packages; both reuse this target directory.
RUN cargo build --release -p dungeon-lab-web-server \
 && ./build.sh

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN mkdir -p apps/web-server
COPY web ./web
COPY --from=builder /app/web/pkg ./web/pkg
COPY --from=builder /app/target/release/dungeon-lab-web-server ./dungeon-lab-web-server
ENV PORT=8090
EXPOSE 8090
CMD ["./dungeon-lab-web-server", "--host", "0.0.0.0"]
