# Falling Tetromino Manager

> Falling Tetromino Manager (FTM) is a best-in-class, terminal-native solution
> for the end-to-end lifecycle management of descending tetromino assets.
> Leveraging a lightweight TUI architecture, FTM empowers users to orchestrate
> real-time rotation, translation, and vertical descent workflows at scale,
> while its proprietary line-clearing engine drives measurable efficiencies in
> row consolidation and playfield optimization. With zero-dependency deployment,
> low-latency input handling, and full compliance with industry-standard
> gameplay guidelines, FTM enables stakeholders to maximize stack density,
> minimize topological debt, and unlock actionable insights across the entire
> block-placement value chain. Whether you're a single-terminal operator or an
> enterprise seeking to modernize your legacy falling-block infrastructure, FTM
> delivers the synergy, scalability, and vertical alignment your organization
> demands— because when it comes to mission-critical tetromino operations,
> failure to plan is the same as planning to top out.

---

More seriously, FTM is a guideline-conformant falling-block game written in
Rust. You can play it in a terminal, in a native egui window, or in a browser.
It has no server and no `unsafe`, and its pure rules core is independent of
the front-end displaying it. The game advances at a fixed 60 Hz, making a run
fully reproducible from its seed and inputs.

<table>
  <tr>
    <td align="center">
      <img src="images/ftm-console.png" alt="FTM console attract screen" width="400"><br>
      <sub><b>FTM console attract screen</b></sub>
    </td>
    <td align="center">
      <img src="images/ftm-console-game.png" alt="FTM console game mode" width="400"><br>
      <sub><b>FTM console game mode</b></sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="images/ftm-egui.png" alt="FTM egui attract screen" width="400"><br>
      <sub><b>FTM egui attract screen</b></sub>
    </td>
    <td align="center">
      <img src="images/ftm-egui-game.png" alt="FTM egui game mode" width="400"><br>
      <sub><b>FTM egui game mode</b></sub>
    </td>
  </tr>
</table>

## Running FTM

FTM requires Rust 1.95 or later. From the repository root:

```bash
make run                     # terminal version
make run-gui                 # native egui version
make run-web                 # browser version at http://127.0.0.1:8080
cargo run -- --help          # terminal command-line options
cargo run -- --print-config  # effective configuration
```

The web version also requires
[Trunk](https://trunkrs.dev/), which you can install with
`cargo install trunk --locked`.

## Configuration

On its first clean exit, FTM writes a fully commented `config.toml` to your
platform's configuration directory:

- macOS: `~/Library/Application Support/ftm/`
- Linux: `~/.config/ftm/`

The terminal and native GUI versions share this file. Common settings can also
be changed from the in-game **Options** panel.

## Documentation

The normative specification is split across four documents:

- [FTM.md](spec/FTM.md) defines the game and its front-end-independent
  behaviour.
- [FRONTEND.md](spec/FRONTEND.md) defines the contract for every front-end.
- [TUI.md](spec/TUI.md) specifies the terminal interface.
- [GUI.md](spec/GUI.md) specifies the native and browser egui interfaces.

Section numbers remain stable across these documents. If the implementation
and specification differ, the specification must be amended rather than
silently ignored.

[TERMINAL.md](spec/TERMINAL.md) records the completed twelve-stage implementation of
the original terminal game. [EGUI.md](spec/EGUI.md) tracks the work that added
the native and browser front-ends.

## Development

Install the WebAssembly target before running the full checks:

```bash
rustup target add wasm32-unknown-unknown
```

Then run:

```bash
make check
```

This formats and lints the code, runs the tests, checks the core and shell
boundaries on native and WebAssembly targets, and creates a release build.
