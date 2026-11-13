# syntax=docker/dockerfile:1
#
# Multi-stage and cache friendly:
#   * tools (cargo-chef, wasm-bindgen, topcoat) are installed in their own stages and rebuild only when a pinned version changes;
#   * cargo-chef cooks the dependencies from a recipe of the manifests, so editing source code never recompiles them;
#   * the site (native, the `website` profile: size over speed, see Cargo.toml) and the lab's WebAssembly module build in parallel stages;
#   * every cargo call has --locked, and every tag and version is pinned (scripts/check-pins.sh looks for newer ones every week).

FROM rust:1.99.1-bookworm AS base
RUN rustup target add wasm32-unknown-unknown
WORKDIR /app

# ---- tools -------------------------------------------------------------------------------------------------
FROM base AS tool-chef
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked cargo install cargo-chef --version 0.1.77 --locked

FROM base AS tool-wasm-bindgen
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked cargo install wasm-bindgen-cli --version 0.2.130 --locked

FROM base AS tool-topcoat
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked cargo install topcoat-cli --version 0.12.0 --locked

# ---- recipe: changes only when manifests or Cargo.lock change ------------------------------------------------
FROM tool-chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- the site: native binary + bundled assets --------------------------------------------------------------
FROM tool-chef AS site-build
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked cargo chef cook --profile website --locked --package tapiti-site --recipe-path recipe.json
COPY --from=tool-topcoat /usr/local/cargo/bin/topcoat /usr/local/cargo/bin/cargo-topcoat /usr/local/cargo/bin/
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo build --profile website --locked -p tapiti-site \
 && topcoat asset bundle --profile website --package tapiti-site

# ---- the lab: WebAssembly module (builds in parallel with the site) -----------------------------------------
FROM tool-chef AS wasm-build
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked cargo chef cook --release --locked --target wasm32-unknown-unknown --package tapiti-wasm --recipe-path recipe.json
COPY --from=tool-wasm-bindgen /usr/local/cargo/bin/wasm-bindgen /usr/local/cargo/bin/
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked scripts/build-wasm.sh

# ---- runtime ---------------------------------------------------------------------------------------------
FROM debian:bookworm-20261104-slim AS runtime
WORKDIR /app
RUN useradd --system --no-create-home --uid 10001 tapiti
COPY --from=site-build /app/target/website/tapiti-site ./tapiti-site
COPY --from=site-build /app/target/website/assets ./assets
COPY --from=wasm-build /app/apps/site/pkg ./pkg
USER tapiti
# `topcoat::start` reads HOST and PORT; the lab's WebAssembly module is served from TAPITI_LAB_PKG.
ENV HOST=0.0.0.0 \
    PORT=8090 \
    TAPITI_LAB_PKG=/app/pkg
EXPOSE 8090
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["./tapiti-site", "--healthcheck"]
CMD ["./tapiti-site"]
