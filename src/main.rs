use anyhow::Result;

fn parse_setup(arg: Vec<String>) -> Result<Vec<usize>> {
    if arg.is_empty() {
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

#[derive(Debug)]
struct Position {
    x: i32,
    y: i32,
}

#[derive(Debug)]
struct Resolution {
    width: i32,
    height: i32,
}

#[derive(Debug)]
struct Output {
    name: String,
    make: String,
    model: String,
    current_resolution: Resolution,
    preferred_resolution: Resolution,
    position: Position,
}

trait Ipc {
    fn get_outputs(&mut self) -> Result<Vec<Output>>;
    fn activate_output(&mut self, output: &Output, new_position: Option<Position>) -> Result<()>;
    fn disable_output(&mut self, output: &Output) -> Result<()>;
}

#[derive(Debug)]
struct SwayIPC {
    connection: swayipc::Connection,
}

impl SwayIPC {
    fn new() -> Result<Self> {
        Ok(Self {
            connection: swayipc::Connection::new()?,
        })
    }

    fn get_preferred_resolution(output: &swayipc::Output) -> Resolution {
        output
            .modes
            .first()
            .map(|mode| Resolution {
                width: mode.width,
                height: mode.height,
            })
            .unwrap_or(Resolution {
                width: output.rect.width,
                height: output.rect.height,
            })
    }
}

impl Ipc for SwayIPC {
    fn get_outputs(&mut self) -> Result<Vec<Output>> {
        let mut outputs = self.connection.get_outputs()?;
        outputs.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(outputs
            .into_iter()
            .map(|output| {
                let preferred_resolution = Self::get_preferred_resolution(&output);
                Output {
                    name: output.name,
                    make: output.make,
                    model: output.model,
                    current_resolution: Resolution {
                        width: output.rect.width,
                        height: output.rect.height,
                    },
                    preferred_resolution,
                    position: Position {
                        x: output.rect.x,
                        y: output.rect.y,
                    },
                }
            })
            .collect())
    }

    fn activate_output(&mut self, output: &Output, new_position: Option<Position>) -> Result<()> {
        let position = new_position.as_ref().unwrap_or(&output.position);
        self.connection.run_command(format!(
            "output {} enable pos {} {}",
            output.name, position.x, position.y
        ))?;
        Ok(())
    }

    fn disable_output(&mut self, output: &Output) -> Result<()> {
        self.connection
            .run_command(format!("output {} disable", output.name))?;
        Ok(())
    }
}

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let setup = parse_setup(args.into_iter().skip(1).collect())?;

    let mut ipc = SwayIPC::new()?;
    let outputs = ipc.get_outputs()?;

    println!("Recognised screens:");
    for (i, output) in outputs.iter().enumerate() {
        println!(
            "{}: {} (current {}×{}) (preferred {}×{}) [{} {}]",
            i,
            output.name,
            output.current_resolution.width,
            output.current_resolution.height,
            output.preferred_resolution.width,
            output.preferred_resolution.height,
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

    if !setup.is_empty() {
        for (i, output) in outputs.iter().enumerate() {
            if !setup.contains(&i) {
                println!("Disabling {}.", output.name);
                ipc.disable_output(output)?;
            }
        }
    }

    let mut x: i32 = 0;
    for screen in setup.iter() {
        let output = &outputs[*screen];
        ipc.activate_output(output, Some(Position { x, y: 0 }))?;
        println!(
            "{}: Setting output {} with rect {}x{} to {}.",
            screen,
            output.name,
            output.preferred_resolution.width,
            output.preferred_resolution.height,
            x
        );
        x += output.preferred_resolution.width;
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
