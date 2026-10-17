#!/usr/bin/env bash
# Prepares a checkout of an engine tag for `cargo publish --workspace`, in the release workflow.
#
# - Writes the tag's version over the 0.0.0 placeholder in Cargo.toml: the package version and
#   the version of every internal path dependency. Nothing is committed back.
# - Writes `exclude=<flags>` to $GITHUB_OUTPUT: the workspace members that are not crates
#   (`publish = false`) and the crates already on crates.io at this version, so a run that
#   failed halfway publishes only what is missing.
#
#   TAG=workspace-v0.43.1 scripts/prepare_crates_release.sh
set -euo pipefail

if [[ ! "${TAG:-}" =~ ^workspace-v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "::error::expected an engine tag such as workspace-v0.43.1, got '${TAG:-}'"
  exit 1
fi
version=${TAG#workspace-v}

grep -q '"0\.0\.0"' Cargo.toml || { echo "::error::no version placeholder in Cargo.toml"; exit 1; }
sed -i "s/\"0\.0\.0\"/\"$version\"/g" Cargo.toml

exclude=""
while IFS=$'\t' read -r name publishable; do
  if [ "$publishable" = false ]; then
    exclude="$exclude --exclude $name"
    continue
  fi
  code=$(curl -s -o /dev/null -w '%{http_code}' \
    -A "triplex release workflow (github.com/triplexanalytics/triplex)" \
    "https://crates.io/api/v1/crates/$name/$version")
  if [ "$code" = 200 ]; then
    echo "$name $version is already on crates.io, skipping it"
    exclude="$exclude --exclude $name"
  fi
done < <(cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[] | [.name, (if .publish == [] then "false" else "true" end)] | @tsv')

echo "version $version, excluded:${exclude:- none}"
echo "exclude=$exclude" >> "${GITHUB_OUTPUT:-/dev/stdout}"
