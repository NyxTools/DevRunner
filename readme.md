# DevRunner

A terminal-based task runner for developers, written in Rust.

## Features
- **Auto-discovery**: Automatically finds `package.json` scripts and `Cargo.toml` targets.
- **TUI**: Simple terminal user interface to select and run tasks.
- **Global Install**: Run it from anywhere.

## Installation

### Prerequisites
- Rust and Cargo installed ([rustup.rs](https://rustup.rs/))

### Option 1: Install from Source (Recommended)
If you have the code locally (e.g., after cloning the repo), run this inside the project folder:

```bash
cargo install --path .
```
*Note: `.` refers to the current directory where `Cargo.toml` is located.*

### Option 2: Install directly from Git
You can install it directly without cloning manually:

```bash
cargo install --git https://github.com/yourusername/devrunner
```

### Option 3: Pre-built Binary
(If you release binaries on GitHub, users can download them directly)

## Usage

Navigate to any project directory and run:

```bash
devrunner
```

### Options

- **Scan a specific path**:
  ```bash
  devrunner --path /path/to/my/project
  ```

- **Specify config file**:
  ```bash
  devrunner --config my-custom-config.json
  ```

## Configuration

You can create a `.devrunner.json` or `.devrunner.toml` file in your project root to customize behavior.

**Example `.devrunner.json`:**
```json
{
  "ignore_paths": ["vendor", "legacy"],
  "custom_scripts": [
    {
      "name": "Deploy to Staging",
      "command": "./deploy.sh staging"
    }
  ]
}
```
