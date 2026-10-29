A desktop-focused Navidrome client written in Rust using egui. This project is heavily inspired by [Feishin](https://github.com/jeffvli/feishin) and can be considered a direct Rust rewrite, with an emphasis on native desktop performance and usability.

**Credits:** Based on Feishin, originally developed by [jeffvli](https://github.com/jeffvli).

**AI Disclosure:** This project was developed entirely using AI-generated code, with the direction, requirements, and design decisions guided by the maintainer.

This is an independent project and is not officially affiliated with Feishin.

P01 now provides a Rust/egui feasibility workspace with native, browser and remote shells. P02 now adds [portable domain contracts and bounded non-secret persistence](docs/app/p02/README.md) under an explicit sequencing exception. It is not a working music client; P00 and P01 acceptance gates remain open. Start with the [P01 build/run guide](docs/app/p01/README.md). See the [P00 evidence and open gates](docs/app/baseline/README.md), [application documentation](docs/app/README.md) and [third-party notices](THIRD_PARTY_NOTICES.md).

The desktop can optionally [load and save isolated preferences](docs/app/p02/desktop-preferences.md) with `--profile-root /absolute/development-root`. Its library table remains a synthetic demo.
