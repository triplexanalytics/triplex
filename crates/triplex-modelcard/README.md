# triplex-modelcard

ModelCard: fairness and slice metrics for a binary classifier, computed from a predictions file (true label, prediction, optional score). It never loads a model. Output is a deterministic JSON card, an offline HTML report and a Python script that reproduces the numbers.

A model card describes measured behavior on the data given. It is not a conformity assessment.

Part of [Triplex](https://github.com/triplexanalytics/triplex), local-first data privacy tooling:
everything runs where you run it, and no crate makes a network request.

Most users want the `triplex` command-line tool (`cargo install triplex`); this crate is for
building on the engine directly. Guides and the glossary are on
[triplexanalytics.io/docs](https://triplexanalytics.io/docs/).

Licensed under [Apache-2.0](https://github.com/triplexanalytics/triplex/blob/main/LICENSE).
