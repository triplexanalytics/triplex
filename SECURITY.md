# Security policy

Triplex handles sensitive data, so we treat security reports as a priority.

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub:
[Report a vulnerability](https://github.com/triplexanalytics/triplex/security/advisories/new).
Do not open a public issue for a security problem.

Include what you found, how to reproduce it, and which version or commit you tested. Use
synthetic data in your report. Never send real personal data, mapping files or credentials.

## What counts

- Any path by which data leaves the machine or the browser tab. Triplex is designed to make no
  network request with your data, so a request you did not expect is a vulnerability.
- Weaknesses in pseudonymization, in the mapping file encryption or in the handling of salts and
  passphrases.
- Errors in the differentially private mode that weaken its stated guarantee.
- Reports or risk scores that state a stronger privacy property than the output has.

## Supported versions

Fixes are made on the latest release.
