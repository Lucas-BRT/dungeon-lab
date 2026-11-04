FROM rust:1-bookworm AS build
RUN rustup target add wasm32-unknown-unknown \
 && cargo install wasm-bindgen-cli --version 0.2.130 --locked
# CARGO_MANIFEST_DIR is baked into the server binary: it finds web/ there, so build and run in /app.
WORKDIR /app
COPY . .
RUN ./build.sh && cargo build --release --bin serve

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=build /app/web ./web
COPY --from=build /app/target/release/serve ./serve
ENV PORT=8090
EXPOSE 8090
CMD ["./serve", "--host", "0.0.0.0"]
