# Contributing to `pythia-scry-gui`

This tool is written in Rust, using the Dioxus cross-platform UI library.
While it is meant primarily for desktop use, please take care to ensure all
functionality is available in both desktop (`dx build --desktop`) and web /
full-stack (`dx build --web`) builds.

Please make all changes in a branch, and provide PRs to the upstream repo.

## Development

### Requirements

- [Rust](https://rustup.rs/)
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started)
- Docker (for running workflows)

### Running the App

The `dx serve` command will run the app and provides auto-refresh functionality when the code changes.

#### Desktop

```bash
dx serve
```

#### Web

```bash
dx serve --platform web
```

After launching the server, open your browser to `http://localhost:8080` (or the URI printed to the console.)