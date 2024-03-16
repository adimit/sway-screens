use anyhow::Result;
use std::fmt::{self, Debug};

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Resolution {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Copy, Clone)]
pub struct Mode {
    pub resolution: Resolution,
    pub refresh: i32,
    pub preferred: bool,
}

#[derive(Debug)]
pub struct Output {
    pub name: String,
    pub enabled: bool,
    pub description: String,
    pub current_mode: Option<Mode>,
    pub preferred_mode: Option<Mode>,
    pub modes: Vec<Mode>,
    pub position: Option<Position>,
    pub scale: f64,
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use colored::Colorize;
        let indicator = {
            if self.enabled {
                "⯀ ".bright_green()
            } else {
                "⮽ ".red()
            }
        };
        write!(f, "{}", indicator)?;
        write!(f, "{}", self.name)?;
        if (self.scale - 1.0).abs() > f64::EPSILON {
            write!(f, " (×{:.2})", self.scale)?;
        }
        if let Some(current_mode) = &self.current_mode {
            write!(f, " {}", current_mode)?;
        }
        if let Some(position) = &self.position {
            if position.x != 0 || position.y != 0 {
                write!(f, " +{},{}", position.x, position.y)?;
            }
        }
        write!(f, ", {} modes", self.modes.len())?;
        write!(f, " [{}]", self.description)?;
        Ok(())
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}×{}", self.width, self.height)
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use colored::Colorize;
        write!(f, "{}", self.resolution)?;
        if self.refresh != 0 {
            write!(f, "@{:.2}kHz", (self.refresh as f64 / 1000.0))?;
        }
        let heart = if self.preferred {
            "♥".green()
        } else {
            " ".clear()
        };
        write!(f, "{}", heart)?;
        Ok(())
    }
}

pub trait OutputManager {
    fn get_outputs(&self) -> Result<Vec<Output>>;
    fn enable_output(&self, output: &Output, position: &Position) -> Result<()>;
    fn disable_output(&self, output: &Output) -> Result<()>;
}
