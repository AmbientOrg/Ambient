# Tilt Runtime

[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/TiltOrg/Tilt#license)
[![Discord](https://img.shields.io/discord/691052431525675048.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/gYSM4tHZ)

Tilt Runtime provides a programming environment for building high performance games and 3d applications.

Our goal is to provide a free and Open source game development API/runtime which can be accessed from any language, which can be run on as many platforms as possible and which is multiplayer native. Since Tilt is powered by WASM, modules built on Tilt are always safe to run, both on your own game servers and on your clients machines.

## Roadmap & features

Note; Tilt is in an alpha stage and will still be iterated on heavily. We're working towards a stable release, but that will happen once the API has started to stabilize.

| Feature | Status |
| ------- | ------ |
| ECS | ✅ |
| WASM API (Rust as client language) | ✅ |
| Multiplayer/networking | ✅ |
| Gpu driven renderer | ✅ |
| FBX & GLTF loading | ✅ |
| Physics (through Physx) | ✅ |
| Animations | ✅ |
| Skinmeshing | ✅ |
| Shadow maps | ✅ |
| Decals | ✅ |
| Multi platform (Windows, Mac, Linux so far) | ✅ |
| Run on Web | 🚧 |
| Client side scripting | 🚧 |
| UI | 🚧 |
| Custom shaders | 🚧 |
| Audio | 🚧 |

## Examples

Each example in the [examples](./examples/) directory can be run with e.g. `tilt run --project_path=examples/hello_world`.

## Installing

You need [Rust](https://www.rust-lang.org/) to install the Tilt runtime. Then run:

```sh
cargo install tilt
```

### Dependencies: Linux/Ubuntu

```sh
apt-get install -y build-essential cmake pkg-config libfontconfig1-dev clang libasound2-dev ninja-build
```

### Running on headless Linux/Ubunutu

```sh
add-apt-repository ppa:oibaf/graphics-drivers -y
apt-get update
apt install -y libxcb-xfixes0-dev mesa-vulkan-drivers
```

## Contributing

We welcome community contributions to this project.

Please talk with us on Discord beforehand if you'd like to contribute a larger piece of work.

## License (MIT)

Tilt is licensed under MIT. See [LICENSE.md](./LICENSE.md)
