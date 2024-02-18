use itertools::Itertools;
use swayipc;

use anyhow::Result;

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let setup = parse_setup(args.into_iter().skip(1).collect())?;

    let mut con = swayipc::Connection::new()?;
    let outputs = con.get_outputs()?;

    if setup.len() > outputs.len() {
        return Err(anyhow::anyhow!(
            "Can't set {} outputs when we only have these: {}",
            setup.len(),
            outputs.into_iter().map(|o| o.name).join(", ")
        ));
    }

    let mut x: i32 = 0;
    for screen in setup.iter() {
        let output = &outputs[*screen];
        con.run_command(format!("output {} enable pos {} y", output.name, x))?;
        println!(
            "{}: Set output {} with rect {}x{} to {}",
            screen, output.name, output.rect.width, output.rect.height, x
        );
        x += output.rect.width;
    }

    for (i, output) in outputs.iter().enumerate() {
        if !setup.contains(&i) {
            con.run_command(format!("output {} disable", output.name))?;
            println!("Disabling {}", output.name)
        }
    }

    Ok(())
}

fn parse_setup(arg: Vec<String>) -> Result<Vec<usize>> {
    if arg.len() != 1 {
        return Err(anyhow::anyhow!("Invalid args."));
    }

    let screens = arg[0]
        .chars()
        .map(|c| {
            c.to_digit(10)
                .map(|i| i as usize)
                .ok_or(anyhow::anyhow!("char '{}' not a digit", c))
        })
        .collect::<Result<Vec<usize>>>()?;

    if screens.len() == 0 {
        Ok(vec![0])
    } else {
        Ok(screens)
    }
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
