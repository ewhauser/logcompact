#!/usr/bin/env python3
import pathlib
import re


root = pathlib.Path(__file__).resolve().parents[1]
errors: list[str] = []
workflows = root / ".github" / "workflows"

for workflow in workflows.glob("*.yml"):
    source = workflow.read_text()
    if not re.search(r"(?m)^permissions: \{\}$", source):
        errors.append(f"{workflow.name}: workflow permissions must default to none")
    if "timeout-minutes:" not in source:
        errors.append(f"{workflow.name}: missing job timeout")
    for dangerous_trigger in ("pull_request_target", "workflow_run"):
        if re.search(rf"(?m)^\s*{dangerous_trigger}\s*:", source):
            errors.append(f"{workflow.name}: forbidden trigger: {dangerous_trigger}")
    for line in source.splitlines():
        if "uses:" not in line:
            continue
        reference = line.split("uses:", 1)[1].split("#", 1)[0].strip()
        if not reference.startswith("./") and not re.search(
            r"@[0-9a-f]{40}$", reference
        ):
            errors.append(f"{workflow.name}: action is not pinned: {reference}")
    if "actions/checkout" in source and "persist-credentials: false" not in source:
        errors.append(f"{workflow.name}: checkout credentials are not disabled")

publish_source = (workflows / "publish.yml").read_text()
if "secrets.CARGO_REGISTRY_TOKEN" in publish_source:
    errors.append("publish.yml: long-lived crates.io credentials are forbidden")
if "rust-lang/crates-io-auth-action@" not in publish_source:
    errors.append("publish.yml: crates.io publishing must use trusted publishing")
if re.search(r"(?m)^\s+uses: actions/cache@|^\s+cache:\s*", publish_source):
    errors.append("publish.yml: publishing must not restore mutable caches")

dependabot_source = (root / ".github" / "dependabot.yml").read_text()
if dependabot_source.count("open-pull-requests-limit: 0") != 2:
    errors.append("dependabot.yml: version update pull requests must be disabled")
if dependabot_source.count("applies-to: security-updates") != 2:
    errors.append("dependabot.yml: every dependency group must be security-only")
if dependabot_source.count("default-days: 7") != 2:
    errors.append("dependabot.yml: every dependency source needs a seven-day cooldown")

if errors:
    raise SystemExit("\n".join(errors))

print("workflow supply-chain policy passed")
