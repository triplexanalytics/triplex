# The `triplex` command-line tool as a container image.
#
#   docker build -t triplex --build-arg TRIPLEX_VERSION=$(git describe --tags --match 'workspace-v*' | sed 's/^workspace-v//') .
#   docker run --rm --network none --user "$(id -u):$(id -g)" -v "$PWD:/data" triplex schema sample.csv
#
# The tool makes no network request, so `--network none` costs nothing and lets Docker enforce it.

FROM rust:1-slim-bookworm AS build
# The version `triplex --version` prints. The build has no git history to read it from.
ARG TRIPLEX_VERSION=
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p triplex

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/triplex /usr/local/bin/triplex
COPY LICENSE NOTICE /usr/share/doc/triplex/
WORKDIR /data
ENTRYPOINT ["triplex"]
