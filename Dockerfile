FROM rust:1.62-slim AS build
RUN useradd --create-home --uid 1000 build && mkdir /app && chown build:build /app
WORKDIR /app
USER build
COPY --chown=build:build . .
RUN \
	--mount=type=cache,id=cargo,target=/usr/local/cargo/registry,sharing=locked,uid=1000 \
	--mount=type=cache,id=confesi-server-build,target=/app/target,sharing=locked,uid=1000 \
	["cargo", "build", "--color=always"]
RUN --mount=type=cache,id=confesi-server-build,target=/app/target,ro,sharing=locked,uid=1000 \
	["cp", "target/debug/confesi-server", "confesi-server"]

FROM rust:1.62-slim AS lint
RUN useradd --create-home --uid 1000 build && mkdir /app && chown build:build /app
WORKDIR /app
USER build
RUN ["rustup", "component", "add", "clippy", "rustfmt"]
COPY --chown=build:build . .
RUN \
	--mount=type=cache,id=cargo,target=/usr/local/cargo/registry,sharing=locked,uid=1000 \
	--mount=type=cache,id=confesi-server-build,target=/app/target,sharing=locked,uid=1000 \
	["cargo", "clippy", "--color=always"]

FROM debian:bullseye-slim
WORKDIR /app
COPY --from=build /app/confesi-server /app/Cargo.lock ./
CMD ["./confesi-server"]
