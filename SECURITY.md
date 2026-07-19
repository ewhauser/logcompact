# Security policy

Please report suspected vulnerabilities privately through GitHub's
**Security** tab using a private vulnerability report. Do not open a public
issue containing exploit details, credentials, or sensitive logs.

Security-sensitive areas include:

- redaction before return, serialization, metadata, or telemetry;
- terminal control-character sanitization;
- bounded memory use under adversarial or untrusted input;
- escaping in GitHub and SARIF output;
- filesystem-free path mapping; and
- release and dependency supply-chain configuration.

Only the latest released version is supported with security fixes.

## Supply-chain controls

GitHub Actions workflows use commit-pinned actions, default-deny permissions,
job-scoped credentials, and checkouts that do not retain credentials. CI rejects
dangerous triggers, mutable action references, release caches, and long-lived
crates.io credentials. Routine dependency updates use a seven-day cooldown, and
pull requests introducing known moderate-or-higher vulnerabilities fail review.

Crates are published from the protected `crates-io` environment with a
short-lived credential obtained directly from crates.io through GitHub OIDC.
No crates.io API token is stored in the repository.
