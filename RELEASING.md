# Releasing

Releases publish the same version to crates.io and PyPI and attach Python artifacts to a GitHub release.

## One-time repository setup

1. Add the GitHub Actions secret `CARGO_REGISTRY_TOKEN` with a crates.io publishing token.
2. Configure a PyPI trusted publisher for repository `IsaacBreen/leveled-gss`, workflow `release.yml`, environment `pypi`.
3. Protect the `pypi` GitHub environment if release approval is desired.

PyPI supports a pending trusted publisher for the first upload of a new project.

## Release checklist

1. Update `CHANGELOG.md`.
2. Set the package version in `Cargo.toml`. Python metadata derives its version from Cargo.
3. Run the complete validation in `CONTRIBUTING.md`.
4. Commit and push the release preparation.
5. Create and push an annotated tag matching the Cargo version:

   ```bash
   git tag -a v0.1.0 -m "leveled-gss 0.1.0"
   git push origin v0.1.0
   ```

The release workflow verifies the tag/version match, publishes the crate, builds Python wheels and an sdist, publishes them to PyPI through trusted publishing, and creates the GitHub release.
