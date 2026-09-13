FROM rust:1.85-bookworm AS build
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=build /app/target/release/semver /usr/local/bin/semver
COPY --from=build /app/target/release/semver-rpc /usr/local/bin/semver-rpc
RUN groupadd --system --gid 10001 app \
    && useradd --system --uid 10001 --gid app --no-create-home --shell /usr/sbin/nologin app
USER 10001
ENTRYPOINT ["semver"]
