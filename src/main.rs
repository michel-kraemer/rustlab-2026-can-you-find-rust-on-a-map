use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod hunt;
mod prepare;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract possible pirate dens and ice cream parlors from an OSM PBF file
    /// and write them to a GeoJSON file
    Prepare {
        /// Input OpenStreetMap PBF file
        input: PathBuf,

        /// Output GeoJSON file
        output: PathBuf,
    },

    /// Perform the actual treasure hunt
    Hunt {
        /// Input GeoJSON file containing possible pirate dens and ice cream
        /// parlors
        input: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Prepare { input, output } => prepare::run(&input, &output),
        Commands::Hunt { input } => hunt::run(&input),
    }
}
