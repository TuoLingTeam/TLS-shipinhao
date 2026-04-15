use anyhow::Result;

fn main() -> Result<()> {
    let command = std::env::args().nth(1).unwrap_or_else(|| "release".into());
    match command.as_str() {
        "release" => {
            println!("release");
        }
        "manifest" => {
            println!("manifest");
        }
        "desktop-build" => {
            println!("desktop-build");
        }
        other => {
            println!("unknown:{other}");
        }
    }
    Ok(())
}
