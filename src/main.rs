use curseclient_rust::{getmodfiles, getmodslistjson};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut query = String::new();
    print!("search for mod: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut query)?;
    let query = query.trim();

    let mods_json = getmodslistjson(query).await?;
    println!("{}", mods_json);

    let mut dllink = String::new();
    print!("enter dllink: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut dllink)?;
    let dllink = dllink.trim();

    let files = getmodfiles(dllink).await?;
    println!("{}", serde_json::to_string_pretty(&files)?);

    Ok(())
}
