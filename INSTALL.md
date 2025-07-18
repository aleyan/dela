# Installation

There are several ways to install `dela`:

## From Source (using Cargo)

```sh
cargo install dela
dela init
```

## Pre-built Binaries

You can download pre-built binaries for your platform from the [GitHub Releases page](https://github.com/yourusername/dela/releases).

### Linux (x86_64)
```sh
# Download the latest release
curl -L https://github.com/yourusername/dela/releases/latest/download/dela-linux-amd64.tar.gz | tar xz
# Move the binary to your PATH
sudo mv dela /usr/local/bin/
```

### Linux (ARM64)
```sh
curl -L https://github.com/yourusername/dela/releases/latest/download/dela-linux-arm64.tar.gz | tar xz
sudo mv dela /usr/local/bin/
```

### macOS (Intel)
```sh
curl -L https://github.com/yourusername/dela/releases/latest/download/dela-darwin-amd64.tar.gz | tar xz
sudo mv dela /usr/local/bin/
```

### macOS (Apple Silicon)
```sh
curl -L https://github.com/yourusername/dela/releases/latest/download/dela-darwin-arm64.tar.gz | tar xz
sudo mv dela /usr/local/bin/
```

### Windows
1. Download `dela-windows-amd64.zip` from the [releases page](https://github.com/yourusername/dela/releases)
2. Extract the archive
3. Add the extracted directory to your PATH

## Shell Integration

After installation, run:
```sh
dela init
```

This will set up dela's shell integration for your current shell. 