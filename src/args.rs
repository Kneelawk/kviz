use crate::project::{Program, Project, VisualizerEnum};
use crate::visualizer::bars::BarsVisualizerInput;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Commands,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    /// Runs a program defined by the command line arguments.
    Run(ProjectArgs),

    /// Creates a project defined by the command line arguments and writes it to a JSON file.
    CreateProject {
        /// The project file to write the specified program to.
        #[arg(short, long)]
        project_file: PathBuf,

        #[command(flatten)]
        project_args: ProjectOptionArgs,
    },

    /// Runs the visualization in the provided project file.
    RunProject {
        /// The project file to load the program from.
        #[arg(short, long)]
        project_file: PathBuf,

        /// Override the project's specified input file with this one.
        /// Note: this is required if the project does not specify an input file.
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Override the project's specified output file with this one.
        /// Note: this is required if the project does not specify an output file.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Args)]
pub struct ProjectArgs {
    /// The input audio file to visualize.
    #[arg(short, long)]
    pub input: PathBuf,

    /// The output video file to write the visualized audio to.
    #[arg(short, long)]
    pub output: PathBuf,

    #[command(flatten)]
    pub program: ProgramArgs,
}

#[derive(Debug, Clone, Args)]
pub struct ProjectOptionArgs {
    /// The input audio file to visualize.
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// The output video file to write the visualized audio to.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    #[command(flatten)]
    pub program: ProgramArgs,
}

#[derive(Debug, Clone, Args)]
pub struct ProgramArgs {
    /// The width of the output video.
    #[arg(long, default_value = "1920")]
    pub width: u32,

    /// The height of the output video.
    #[arg(long, default_value = "1080")]
    pub height: u32,

    /// The visualizer to use in this program.
    #[command(subcommand)]
    pub visualizer: VisualizerArgs,
}

#[derive(Debug, Clone, Subcommand)]
pub enum VisualizerArgs {
    /// Runs the Bars visualizer, drawing flashing vertical bars on the screen for the different frequencies.
    Bars,
}

impl From<ProjectArgs> for Project {
    fn from(value: ProjectArgs) -> Self {
        Project {
            input: Some(value.input),
            output: Some(value.output),
            program: value.program.into(),
        }
    }
}

impl From<ProjectOptionArgs> for Project {
    fn from(value: ProjectOptionArgs) -> Self {
        Project {
            input: value.input,
            output: value.output,
            program: value.program.into(),
        }
    }
}

impl From<ProgramArgs> for Program {
    fn from(value: ProgramArgs) -> Self {
        Program {
            width: value.width,
            height: value.height,
            visualizer: value.visualizer.into(),
        }
    }
}

impl From<VisualizerArgs> for VisualizerEnum {
    fn from(value: VisualizerArgs) -> Self {
        match value {
            VisualizerArgs::Bars => VisualizerEnum::Bars(BarsVisualizerInput {}),
        }
    }
}
