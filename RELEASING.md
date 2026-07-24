# Releasing

Releases publish the same version to crates.io and PyPI and attach Python artifacts to a GitHub release.

## One-time repository setup

The repository release workflow accepts:

- `CARGO_REGISTRY_TOKEN` for crates.io; and
- either `PYPI_API_TOKEN` or a PyPI trusted publisher for repository `IsaacBreen/weighted-gss`, workflow `release.yml`, environment `pypi`.

Both repository secrets are configured. Trusted publishing remains supported: delete `PYPI_API_TOKEN` after registering the pending publisher on PyPI to switch to short-lived OIDC credentials. Protect the `pypi` GitHub environment if release approval is desired.

## Release checklist

1. Update `CHANGELOG.md`.
2. Set the package version in `Cargo.toml`. Python metadata derives its version from Cargo.
3. Run the complete validation in `CONTRIBUTING.md`.
4. Commit and push the release preparation.
5. Create and push an annotated tag matching the Cargo version:

   ```bash
   git tag -a v0.1.0 -m "weighted-gss 0.1.0"
   git push origin v0.1.0
   ```

The release workflow verifies the tag/version match, publishes the crate, builds Python wheels and an sdist, publishes them to PyPI through trusted publishing, and creates the GitHub release.
