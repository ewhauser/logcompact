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
