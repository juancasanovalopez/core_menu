# Release policy

- Follow Semantic Versioning for this application and Conventional Commits:
  `fix:` is PATCH, `feat:` is MINOR, and `!` or a `BREAKING CHANGE:` footer is
  MAJOR. The highest applicable increment wins. Documentation and maintenance
  alone do not request a release.
- Explicitly declare incompatible changes to CLI behavior, user-facing
  workflows, configuration, or update/asset contracts. Commit syntax cannot
  detect breaking changes automatically.
- Release Please prepares the release PR on `master`. Let its Rust strategy
  synchronize `Cargo.toml`, the local package in `Cargo.lock`,
  `.release-please-manifest.json`, and `CHANGELOG.md`. Do not bump versions for
  ordinary builds or independently edit generated release versions.
- Publish only stable `vMAJOR.MINOR.PATCH` tags, without leading zeroes,
  prereleases, or build metadata. Versions must advance numerically.
- Before tagging, review and merge the release PR, run the checks documented
  in `doc/instalacion.md`, and tag that exact merged commit. Do not tag or publish
  without an explicit user request.
- Preserve the signed release workflow and asset contract. Never overwrite a
  published version or commit private signing keys. Build/sign all six assets
  successfully before marking the release latest.
- See `doc/instalacion.md`, section "Publishing a version", for the canonical
  preparation, publication, and recovery procedure.
