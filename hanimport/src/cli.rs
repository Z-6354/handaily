use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::delegate;
use crate::paths;

#[derive(Parser)]
#[command(
    name = "hanimport",
    version,
    about = "HANDAILY dev importer — unpack game bundles, batch import to hanpet"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build Spine/Cubism model configs (config.json / animations.meta / touch_areas)
    Config {
        /// Model folder or parent (data/live2d or data/model/unpacked)
        #[arg(short, long)]
        input: PathBuf,

        /// Preview only; do not write files
        #[arg(long)]
        dry_run: bool,

        /// Overwrite existing JSON
        #[arg(long)]
        force: bool,
    },

    /// Unity AssetBundle → Spine folder under data/live2d/
    Unpack {
        /// Bundle file or directory to scan
        #[arg(short, long)]
        input: PathBuf,

        /// Output root (default: HANDAILY_LIVE2D_PATH or data/live2d/)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Generate ViewerEX + action meta JSON after spine unpack
        #[arg(long)]
        with_config: bool,

        /// List bundles only; do not write files
        #[arg(long)]
        dry_run: bool,

        /// Parallel workers (default: HANIMPORT_UNPACK_JOBS or 2–8 from CPU)
        #[arg(short = 'j', long)]
        jobs: Option<usize>,
    },

    /// Generate live2d import plan JSON (wraps mcp/blhx-wiki live2d-plan)
    Plan {
        /// Output path (default: data/import/live2d-plan.json)
        #[arg(short, long)]
        out: Option<PathBuf>,

        /// Live2D root override
        #[arg(long)]
        live2d_root: Option<PathBuf>,

        /// Minimum match score (default: 80)
        #[arg(long, default_value_t = 80)]
        min_score: u32,

        /// Include ships without imported persona
        #[arg(long)]
        all_personas: bool,
    },

    /// Batch import Spine models (wraps live2d_import)
    Models {
        /// Plan JSON path
        #[arg(long)]
        plan: Option<PathBuf>,

        /// Live2D root override
        #[arg(long)]
        live2d_root: Option<PathBuf>,

        /// Preview only
        #[arg(long)]
        dry_run: bool,

        /// Loop until done
        #[arg(long)]
        all: bool,

        /// Max items per batch
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Batch import personas from BWIKI SQLite (wraps blhx_import)
    Personas {
        /// Arguments forwarded to blhx_import (e.g. --all --skip-existing --limit 50)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Export/import character roster packs (wraps roster_pack)
    Roster {
        #[command(subcommand)]
        command: RosterCommands,
    },
}

#[derive(Subcommand)]
pub enum RosterCommands {
    /// Export faction character packs to release/
    Export {
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

pub fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Commands::Config {
            input,
            dry_run,
            force,
        } => crate::config::run_config(&input, dry_run, force),
        Commands::Unpack {
            input,
            output,
            dry_run,
            with_config,
            jobs,
        } => {
            let out = paths::resolve_unpack_output(&input, output);
            let jobs = jobs.unwrap_or_else(crate::unpack::default_jobs);
            crate::unpack::run_unpack(&input, &out, dry_run, with_config, jobs)
        }
        Commands::Plan {
            out,
            live2d_root,
            min_score,
            all_personas,
        } => {
            let out_path = out.unwrap_or_else(paths::default_import_plan_path);
            delegate::run_live2d_plan(
                &out_path,
                live2d_root.as_deref(),
                min_score,
                !all_personas,
            )
        }
        Commands::Models {
            plan,
            live2d_root,
            dry_run,
            all,
            limit,
        } => {
            let plan_path = plan.unwrap_or_else(paths::default_import_plan_path);
            let mut args = vec![
                "--plan".to_string(),
                plan_path.display().to_string(),
            ];
            if let Some(root) = live2d_root {
                args.push("--live2d-root".to_string());
                args.push(root.display().to_string());
            }
            if dry_run {
                args.push("--dry-run".to_string());
            }
            if all {
                args.push("--all".to_string());
            }
            if let Some(n) = limit {
                args.push("--limit".to_string());
                args.push(n.to_string());
            }
            delegate::run_hanpet_bin("live2d_import", &args)
        }
        Commands::Personas { args } => {
            if args.is_empty() {
                return delegate::run_hanpet_bin("blhx_import", &["--help".to_string()]);
            }
            delegate::run_hanpet_bin("blhx_import", &args)
        }
        Commands::Roster { command } => match command {
            RosterCommands::Export { output } => {
                let mut args = vec!["export".to_string()];
                if let Some(dir) = output {
                    args.push("--output".to_string());
                    args.push(dir.display().to_string());
                }
                delegate::run_hanpet_bin("roster_pack", &args)
            }
        },
    }
}
