# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by
[SignPath Foundation](https://signpath.org/).

## What is signed

Final releases of Joystick Gobloon published on the
[Releases page](https://github.com/h0useRus/JoystickGobloon/releases):

- `JoystickGobloon.exe` (in the installer and in the portable zip);
- the installer `JoystickGobloon-<version>-setup.exe`.

Pre-release (alpha) builds are not signed.

Every signed file is built from the source code in this repository by GitHub Actions
([release.yml](../.github/workflows/release.yml)) and submitted to SignPath from that workflow;
nothing is built or signed on a personal computer.

The installer also carries the HidHide and vJoy driver installers. They are third-party open source
projects (MIT), downloaded from their official releases with a pinned SHA-256 and signed by their own
authors; this project does not sign them.

SignPath artifact configurations used by the workflow: [app](../.github/signpath/app.xml),
[installer](../.github/signpath/installer.xml).

## Team roles

- Committers and reviewers: [h0use](https://github.com/h0useRus)
- Approvers: [h0use](https://github.com/h0useRus)

Every signing request is approved manually by an approver.

## Privacy policy

This program will not transfer any information to other networked systems unless specifically
requested by the user or the person installing or operating it.

Joystick Gobloon works on the local computer: it reads game controllers, talks to the locally installed
HidHide and vJoy drivers and stores its settings and log in `%APPDATA%\JoystickGobloon`. Links in the
*About* page open in the web browser only when you click them.
