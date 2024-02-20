use swayipc;

use anyhow::Result;

fn get_preferred(output: &swayipc::Output) -> Result<&swayipc::Mode> {
    output
        .modes
        .first()
        .ok_or(anyhow::anyhow!("Output {} has no modes", output.name))
}

fn parse_setup(arg: Vec<String>) -> Result<Vec<usize>> {
    if arg.len() == 0 {
        return Ok(vec![]);
    }

    arg[0]
        .chars()
        .map(|c| {
            c.to_digit(10)
                .map(|i| i as usize)
                .ok_or(anyhow::anyhow!("char '{}' not a digit", c))
        })
        .collect::<Result<Vec<usize>>>()
}

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let setup = parse_setup(args.into_iter().skip(1).collect())?;

    let mut con = swayipc::Connection::new()?;
    let outputs = {
        let mut o = con.get_outputs()?;
        o.sort_by(|a, b| a.name.cmp(&b.name));
        o
    };

    println!("Recognised screens:");
    for (i, output) in outputs.iter().enumerate() {
        let preferred = get_preferred(output)?;
        println!(
            "{}: {} ({}×{}, {}×{}) [{} {}]",
            i,
            output.name,
            output.rect.width,
            output.rect.height,
            preferred.width,
            preferred.height,
            output.make,
            output.model
        );
    }

    if setup.len() > outputs.len() {
        return Err(anyhow::anyhow!(
            "Can't set {} outputs when we only have {}.",
            setup.len(),
            outputs.len()
        ));
    }

    if setup.len() > 0 {
        for (i, output) in outputs.iter().enumerate() {
            if !setup.contains(&i) {
                con.run_command(format!("output {} disable", output.name))?;
                println!("Disabling {}.", output.name)
            }
        }
    }

    let mut x: i32 = 0;
    for screen in &setup {
        let output = &outputs[*screen];
        let preferred = get_preferred(output)?;
        con.run_command(format!(
            "output {} enable pos {} 0 res {} {}",
            output.name, x, preferred.width, preferred.height
        ))?;
        println!(
            "Setting {}: output {} to {}×{} at {}.",
            screen, output.name, preferred.width, preferred.height, x
        );
        x += preferred.width;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_setup() {
        let args = vec![String::from("012")];
        let result = parse_setup(args).unwrap();
        assert_eq!(result, &[0, 1, 2]);
    }
}
