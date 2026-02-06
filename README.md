# glimpse

A Windows Shell Extension that shows thumbnail previews of `.gltf`, `.glb`, `.bbmodel`, and Vintage Story `.json` model files directly in File Explorer.

![License](https://img.shields.io/badge/license-MIT-blue)
![Platform](https://img.shields.io/badge/platform-Windows-0078D4)
![Rust](https://img.shields.io/badge/rust-stable-orange)

## Supported Formats

| Format | Extensions | Description |
|--------|-----------|-------------|
| **glTF / GLB** | `.gltf`, `.glb` | Industry-standard 3D transmission format |
| **Blockbench** | `.bbmodel` | Blockbench model format used for Minecraft modding and more |
| **Vintage Story** | `.json` | JSON-based model format used by Vintage Story |

## Features

- **Native Explorer Integration** — Thumbnails appear just like images, videos, and other supported formats
- **Texture Support** — Renders base color textures with proper UV mapping
- **Vertex Colors** — Displays models with vertex color attributes
- **Multiple Formats** — glTF/GLB, Blockbench .bbmodel, and Vintage Story .json models
- **Software Rendering** — No GPU required, works in VMs and remote desktop
- **Lenient Parsing** — Gracefully handles files with external references by rendering geometry only

## Screenshot

![Example thumbnails in Windows Explorer](example.png)

Models shown are from [FrenchKrab/mc-blockbench-models](https://github.com/FrenchKrab/mc-blockbench-models).

## How It Works

glimpse is a COM DLL implementing Windows' [`IThumbnailProvider`](https://learn.microsoft.com/en-us/windows/win32/api/thumbcache/nn-thumbcache-ithumbnailprovider) interface. When Explorer encounters a supported model file:

1. Windows loads our DLL and passes the file data
2. We parse the model (glTF via the [`gltf`](https://crates.io/crates/gltf) crate, bbmodel via `serde_json`, Vintage Story JSON via `json5`)
3. A software rasterizer renders the scene with:
   - Automatic camera framing based on bounding sphere
   - Flat shading with ambient + diffuse + specular lighting
   - Z-buffer depth testing
   - Per-pixel texture sampling with barycentric UV interpolation
4. The resulting bitmap is returned to Explorer

## Installation

Download `glimpse-setup.exe` from the [releases page](https://github.com/codename-B/glimpse/releases) and run it. See [INSTALLER.md](INSTALLER.md) for details.

### Building from Source

**Prerequisites:** [Rust](https://rustup.rs/) with `stable-x86_64-pc-windows-msvc` toolchain.

```powershell
cargo build --release
```

This produces `target/release/glimpse.dll`.

## Limitations

| Limitation | Description |
|------------|-------------|
| **Triangles Only** | Line, point, and strip primitives are skipped |
| **No Animation** | Static pose only (no skinning, morphs, or animation) |
| **External Resources** | Files referencing external .bin or textures render geometry only |
| **CPU Rendering** | Fast for thumbnails but not real-time |

## Dependencies

- [`gltf`](https://crates.io/crates/gltf) — glTF 2.0 parsing
- [`serde` / `serde_json`](https://crates.io/crates/serde_json) — bbmodel JSON parsing
- [`json5`](https://crates.io/crates/json5) — Vintage Story JSON5 parsing
- [`base64`](https://crates.io/crates/base64) — Data URI decoding
- [`image`](https://crates.io/crates/image) — PNG/JPEG texture decoding
- [`windows`](https://crates.io/crates/windows) — Windows API bindings

## Contributing

Contributions welcome! Please open an issue or PR.

## License

MIT License — see [LICENSE](LICENSE) for details.
