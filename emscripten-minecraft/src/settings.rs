//! Operator settings, read from the process environment before the server
//! starts.

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub port: u16,
    pub view_distance: u8,
    pub simulation_distance: u8,
    pub max_players: u32,
    pub compression_threshold: i32,
    pub compression_level: u32,
    pub motd: String,
    pub seed: Option<String>,
    pub data_dir: String,
}

impl Settings {
    pub fn from_env() -> Result<Settings, String> {
        let var = |name: &str| -> Result<Option<String>, String> {
            match std::env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => Err(format!("{name} must be a string")),
            }
        };
        let integer = |name: &str, fallback: i64, min: i64, max: i64| -> Result<i64, String> {
            let Some(raw) = var(name)? else {
                return Ok(fallback);
            };
            raw.parse::<i64>()
                .ok()
                .filter(|v| (min..=max).contains(v))
                .ok_or_else(|| format!("{name} must be an integer between {min} and {max}"))
        };
        let view_distance = integer("VIEW_DISTANCE", 4, 2, 32)? as u8;
        let simulation_distance = integer("SIMULATION_DISTANCE", 3, 2, 32)? as u8;
        if simulation_distance > view_distance {
            return Err("SIMULATION_DISTANCE must not exceed VIEW_DISTANCE".into());
        }
        let motd = var("MOTD")?.unwrap_or_else(|| "Minecraft on Node.js via Emscripten".into());
        if motd.trim().is_empty() || motd.len() > 512 {
            return Err("MOTD must contain between 1 and 512 characters".into());
        }
        let seed = var("WORLD_SEED")?;
        if let Some(seed) = &seed {
            if seed.trim().is_empty() || seed.len() > 256 {
                return Err(
                    "WORLD_SEED must be a nonempty string of at most 256 characters".into(),
                );
            }
        }
        Ok(Settings {
            port: integer("PORT", 25565, 1, 65535)? as u16,
            view_distance,
            simulation_distance,
            max_players: integer("MAX_PLAYERS", 20, 1, 1000)? as u32,
            compression_threshold: integer("COMPRESSION_THRESHOLD", 512, -1, 2 * 1024 * 1024)?
                as i32,
            compression_level: integer("COMPRESSION_LEVEL", 1, 0, 9)? as u32,
            motd,
            seed,
            data_dir: var("DATA_DIR")?.unwrap_or_else(|| "data".into()),
        })
    }
}
