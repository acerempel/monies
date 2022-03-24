use std::fs::File;
use std::io::{Read, ErrorKind, Write};
use std::path::{PathBuf, Path};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub(crate) struct Config {
    #[serde(default = "Config::default_address")]
    pub(crate) address: [u8; 4],
    #[serde(default = "Config::default_port")]
    pub(crate) port: u16,
    #[serde(default = "Config::default_db_file")]
    pub(crate) db_file: PathBuf,
}

impl Config {
    pub(crate) fn get(config_path: &Path) -> Result<Self, eyre::Report> {
        match File::open(config_path) {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                Ok(toml::de::from_str(&contents)?)
            },
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let config = Config::default();
                let mut file = File::create(config_path)?;
                let ser = toml::ser::to_vec(&config)?;
                file.write_all(&ser)?;
                Ok(config)
            },
            Err(err) => Err(err.into()),
        }
    }
    pub(crate) fn default_db_file() -> PathBuf {
        PathBuf::from("memory")
    }

    pub(crate) fn default_address() -> [u8; 4] {
        [127, 0, 0, 1]
    }

    pub(crate) fn default_port() -> u16 {
        4000
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: Self::default_address(),
            port: Self::default_port(),
            db_file: Self::default_db_file(),
        }
    }
}
