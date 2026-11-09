# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org).

The Windows program (`CVBuilder.exe`) in each release is signed with this certificate. It is built
by GitHub Actions from the source code in this repository
([`.github/workflows/build.yml`](.github/workflows/build.yml)), and nothing built elsewhere is
signed. Every signing request is approved by hand.

The Microsoft Store version is signed by Microsoft. The Mac version is not signed yet.

## Team roles

- Committers and reviewers: [SoloSaftis](https://github.com/SoloSaftis)
- Approvers: [SoloSaftis](https://github.com/SoloSaftis)

Changes to `main` go through pull requests, and the builds must pass before they are merged.
Everyone on the team uses multi-factor authentication for GitHub and SignPath.

## Privacy

This program will not transfer any information to other networked systems unless specifically
requested by the user or the person installing or operating it. See the
[privacy policy](PRIVACY.md) for what is sent when you choose to fetch a job ad or connect an
AI service.
