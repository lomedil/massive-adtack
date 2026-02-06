use crate::config::Config;
use anyhow::Result;

pub fn execute() -> Result<()> {
    let cfg = Config::load()?;
    println!("Configuration found:");
    println!("{:#?}", cfg);
    Ok(())
}
