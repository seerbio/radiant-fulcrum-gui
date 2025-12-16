# Pythia+Scry Workflow GUI

A cross-platform GUI frontend for running Pythia+Scry workflows in a Docker container, built with [Dioxus](https://dioxuslabs.com/) and Rust.

## Running

To run this app, download an appropriate package from the Releases section.

You must also install Docker to allow running the containerized data processing workflow.

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
