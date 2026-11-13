# Both base images are pinned by digest, so a rebuild uses exactly the same
# bases until someone updates them on purpose. The Rust version matches
# app/rust-toolchain.toml; bump both together.

FROM rust:1.100.0-slim-trixie@sha256:e9d95b74296f7126a83ae3d041aeaba1ff4c478f13abc5cc034fcf7c131c3d4a AS build
WORKDIR /src

# Dependencies get their own layer, built against placeholder sources, so a
# code change doesn't rebuild them.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
 && echo 'fn main() {}' > src/main.rs \
 && touch src/lib.rs \
 && cargo build --release --locked \
 && rm -rf src

COPY src src
COPY static static
# The copied sources keep their original timestamps, which can be older than
# the placeholder build; touch them so cargo rebuilds the crate.
RUN touch src/main.rs src/lib.rs \
 && cargo build --release --locked

# Distroless: glibc and CA certificates, but no shell or package manager. The
# nonroot variant already runs as UID 65532; USER states it as a number so
# Kubernetes' runAsNonRoot can verify it.
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2
COPY --from=build /src/target/release/miniarcade /usr/local/bin/miniarcade
USER 65532:65532
EXPOSE 8080 9090
ENTRYPOINT ["/usr/local/bin/miniarcade"]
