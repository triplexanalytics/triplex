# triplex-anonymizer

The Triplex Anonymizer engine. Tier 1 pseudonymizes declared columns (keyed HMAC tokens, format-preserving substitution, bucketing, partial masking, removal) and writes the mapping that reverses it to a separate file. Tier 2 generates synthetic tables from a fitted model (Gaussian copula, discrete Bayesian network or independent marginals), each run with a fidelity report and a measured leakage report.

Pseudonymized data is not anonymous: whoever holds the mapping file can reverse it. Plain synthesis makes no formal privacy claim; read the leakage report of each run.

Part of [Triplex](https://github.com/triplexanalytics/triplex), local-first data privacy tooling:
everything runs where you run it, and no crate makes a network request.

Most users want the `triplex` command-line tool (`cargo install triplex`); this crate is for
building on the engine directly. Guides and the glossary are on
[triplexanalytics.io/docs](https://triplexanalytics.io/docs/).

Licensed under [Apache-2.0](https://github.com/triplexanalytics/triplex/blob/main/LICENSE).
