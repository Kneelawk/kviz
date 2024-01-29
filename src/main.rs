#[macro_use]
extern crate tracing;

use crate::args::Commands;
use crate::project::Project;
use anyhow::{bail, Context};
use args::Cli;
use clap::Parser;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod args;
mod ffmpeg;
mod project;
mod recycle;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();
    ffmpeg::init_ffmpeg()?;

    let args = Cli::parse();

    match args.subcommand {
        Commands::Run(args) => {
            let project: Project = args.into();

            project.visualize().await.context("Running visualization")?;
        }
        Commands::CreateProject {
            project_file,
            project_args,
        } => {
            let project: Project = project_args.into();

            let project_bytes =
                serde_json::to_vec_pretty(&project).context("Serializing project")?;

            let mut open_project_file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&project_file)
                .await
                .context("Opening project file")?;
            open_project_file
                .write_all(&project_bytes)
                .await
                .context("Writing project file")?;

            info!("Project file written to {:?}", &project_file);
        }
        Commands::RunProject {
            project_file,
            input,
            output,
        } => {
            info!("Loading project file from: {:?}", &project_file);

            let mut open_project_file = OpenOptions::new()
                .read(true)
                .open(&project_file)
                .await
                .context("Opening project file")?;
            let mut project_str = String::new();
            open_project_file
                .read_to_string(&mut project_str)
                .await
                .context("Reading project file")?;

            let project_from_file: Project =
                serde_json::from_str(&project_str).context("Deserializing project")?;

            let project = Project {
                input: input.or(project_from_file.input),
                output: output.or(project_from_file.output),
                ..project_from_file
            };

            if project.input.is_none() {
                bail!("Neither project nor arguments provide an input file");
            }

            if project.output.is_none() {
                bail!("Neither project nor arguemnts provide an output file");
            }

            project.visualize().await.context("Running visualization")?;
        }
    }

    info!("Done.");

    Ok(())
}
