# CurseClient-Rust
Rust port of CurseClient, a keyless client for CurseForge.
Originally by [ObjectiveMoon](https://github.com/ObjectiveMoonmc)

## Usage
Library:
```rust
use curseclient_rust::{getmodfiles, getmodslist};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mods = getmodslist("sodium").await?;
    println!("found {} mods", mods.len());

    if let Some(first) = mods.first() {
        let files = getmodfiles(&first.dllink).await?;
        println!("{} files", files.len());
    }

    Ok(())
}
```

For iOS:
```swift
let client = CurseClient()
let modsjson = client.getmodslist(query: "sodium")
let filesjson = client.getmodfiles(dllink: "https://www.curseforge.com/minecraft/mc-mods/.../files/123456")
```

CLI (interactive example):
```bash
cargo run
```

## Build
GitHub Actions builds on every push and uploads `libcurseclient.dylib` to the `latest` release.
