# triplex-driftguard

DriftGuard: compares a candidate table against a baseline and reports schema regressions (a column removed or its type changed) and statistical drift (null rate, mean or type consistency moving past a threshold). Built to run in CI.

Part of [Triplex](https://github.com/triplexanalytics/triplex), local-first data privacy tooling:
everything runs where you run it, and no crate makes a network request.

Most users want the `triplex` command-line tool (`cargo install triplex`); this crate is for
building on the engine directly. Guides and the glossary are on
[triplexanalytics.io/docs](https://triplexanalytics.io/docs/).

Licensed under [Apache-2.0](https://github.com/triplexanalytics/triplex/blob/main/LICENSE).
