# Code signing policy

Windows releases will be signed once the project has a code signing certificate. Until then, no installers are published.

Planned: Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

## Team and roles

- Committers and reviewers: [the repository maintainers](https://github.com/JordanAlbayrak)
- Approvers: [the repository owner](https://github.com/JordanAlbayrak)

Every change from outside the team is reviewed in a pull request before it is merged. Every release is built by GitHub Actions from this repository, and each signing request is approved by hand.

## Privacy

This program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it.

Its only network feature is the update check. It runs when the user presses **Check for updates**, or once a day when the user turns on **Automatic updates**, which is off by default.
