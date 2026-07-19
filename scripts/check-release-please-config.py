#!/usr/bin/env python3
import json
import pathlib
import tomllib


root = pathlib.Path(__file__).resolve().parents[1]
config = json.loads((root / ".release-please-config.json").read_text())
manifest = json.loads((root / ".release-please-manifest.json").read_text())
cargo = tomllib.loads((root / "Cargo.toml").read_text())
cargo_lock = tomllib.loads((root / "Cargo.lock").read_text())
fuzz_cargo_lock = tomllib.loads((root / "fuzz" / "Cargo.lock").read_text())
package_config = config["packages"]["."]
version = manifest["."]

assert set(config["packages"]) == {"."}
assert package_config["release-type"] == "simple"
assert package_config["package-name"] == "logcompact"
assert package_config["version-file"] == "version.txt"
assert package_config["include-component-in-tag"] is False
assert package_config["include-v-in-tag"] is True
assert package_config["bump-minor-pre-major"] is True
assert package_config["draft"] is True
assert package_config["force-tag-creation"] is True
assert (root / "version.txt").read_text().strip() == version
assert cargo["workspace"]["package"]["version"] == version

workspace_crates = {
    "logcompact",
    "logcompact-builtins",
    "logcompact-core",
}
manifest_crates = set()
for crate_manifest in (root / "crates").glob("*/Cargo.toml"):
    package = tomllib.loads(crate_manifest.read_text())["package"]
    if package["name"] in workspace_crates:
        manifest_crates.add(package["name"])
        assert package["version"] == {"workspace": True}
assert manifest_crates == workspace_crates

workspace_dependencies = cargo["workspace"]["dependencies"]
for dependency in ("logcompact-core", "logcompact-builtins"):
    assert workspace_dependencies[dependency]["version"] == version

lock_versions = {
    package["name"]: package["version"]
    for package in cargo_lock["package"]
    if package["name"] in workspace_crates
}
assert lock_versions == dict.fromkeys(workspace_crates, version)

fuzz_workspace_crates = {"logcompact-builtins", "logcompact-core"}
fuzz_lock_versions = {
    package["name"]: package["version"]
    for package in fuzz_cargo_lock["package"]
    if package["name"] in fuzz_workspace_crates
}
assert fuzz_lock_versions == dict.fromkeys(fuzz_workspace_crates, version)

updaters = package_config["extra-files"]
cargo_jsonpaths = {
    updater["jsonpath"]
    for updater in updaters
    if updater["type"] == "toml" and updater["path"] == "Cargo.toml"
}
assert cargo_jsonpaths == {
    "$.workspace.package.version",
    "$['workspace']['dependencies']['logcompact-core']['version']",
    "$['workspace']['dependencies']['logcompact-builtins']['version']",
}

lock_updater = next(
    updater
    for updater in updaters
    if updater["type"] == "toml" and updater["path"] == "Cargo.lock"
)
for package in workspace_crates:
    assert f'@.name.value=="{package}"' in lock_updater["jsonpath"]

fuzz_lock_updater = next(
    updater
    for updater in updaters
    if updater["type"] == "toml" and updater["path"] == "fuzz/Cargo.lock"
)
for package in fuzz_workspace_crates:
    assert f'@.name.value=="{package}"' in fuzz_lock_updater["jsonpath"]

release_docs = (root / "RELEASING.md").read_text()
assert "workflow: `release-please.yml`" in release_docs
assert "workflow: `publish.yml`" in release_docs

print("release-please configuration is consistent")
