use anyhow::Result;

mod screens;
mod setup;
mod wlr;

use crate::{
    screens::{OutputManager, Position},
    setup::parse_setup,
    wlr::WlrOutputManager,
};

fn main() -> Result<()> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::WARN)
            .finish(),
    )?;

    let args = std::env::args().collect::<Vec<String>>();
    let setup = parse_setup(args.into_iter().skip(1).collect())?;

    let man = WlrOutputManager::new()?;
    let outputs = man.get_outputs()?;

    println!("Recognised screens:");
    for (i, output) in outputs.iter().enumerate() {
        println!("{}: {}", i, output);
    }

    if setup.len() > outputs.len() {
        return Err(anyhow::anyhow!(
            "Can't set {} outputs when we only have {}.",
            setup.len(),
            outputs.len()
        ));
    }

    if !setup.is_empty() {
        for (i, output) in outputs.iter().enumerate() {
            if !setup.contains(&i) {
                println!("Disabling {}.", output.name);
                man.disable_output(output)?;
            }
        }
    }

    let mut x: i32 = 0;
    for screen in setup.iter() {
        let output = &outputs[*screen];
        if let Some(mode) = output.preferred_mode.or(output.current_mode) {
            x = mode.resolution.width;
        }
        man.enable_output(output, &Position { x, y: 0 })?;
    }

    Ok(())
}
