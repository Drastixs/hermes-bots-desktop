# Third-party assets and licences

The code of this project is MIT (see `LICENSE`). The binary embeds the files below.

| File | Source | Licence | Licence text |
|---|---|---|---|
| `assets/fonts/Inter-*.ttf` | [Inter](https://github.com/rsms/inter) by Rasmus Andersson | SIL Open Font License 1.1 | `assets/fonts/Inter-OFL.txt` |
| `assets/fonts/JetBrainsMono-Regular.ttf` | [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) | SIL Open Font License 1.1 | `assets/fonts/JetBrainsMono-OFL.txt` |
| `assets/fonts/lucide.ttf` | [Lucide](https://github.com/lucide-icons/lucide) icon font | ISC | `assets/fonts/Lucide-LICENSE.txt` |
| `assets/wallpaper.jpg` | A teal-tinted, darker copy of `ds-assets/filler-bg0.jpg` from [hermes-agent](https://github.com/NousResearch/hermes-agent) | MIT, Copyright (c) 2025 Nous Research | `assets/HERMES-AGENT-LICENSE.txt` |
| Theme colours (`crates/app/src/theme.rs`) | The "Hermes Teal" theme values of hermes-agent | MIT, Copyright (c) 2025 Nous Research | `assets/HERMES-AGENT-LICENSE.txt` |
| `assets/icons/hermes-bots-*.png` | Original art made for this project | MIT (this project) | `LICENSE` |

## Not shipped

The Hermes Webapp uses the display fonts Collapse, Rules and Mondwest. These fonts
are not open source, so this project does not include them. Inter replaces them.

This project has no relation to xAI or Grok, and it does not use their names, logos or assets.

## Rust crates

The Rust dependencies (iced, tokio, reqwest, tokio-tungstenite, keyring, axum and
others) are linked into the binary under their own licences, which are mostly MIT
or Apache-2.0. `Cargo.lock` has the full list. To get a licence report, run
`cargo install cargo-about && cargo about generate about.hbs`, or use `cargo license`.
