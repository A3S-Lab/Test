use std::process::ExitCode;

use a3s_test_agent::{
    contract_generation_provider_schema, visual_grounding_provider_schema, ProviderProtocolSchema,
};
use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Args)]
pub(crate) struct ProviderArgs {
    #[command(subcommand)]
    command: ProviderCommand,
}

#[derive(Debug, Subcommand)]
enum ProviderCommand {
    /// Print one provider protocol and its generated request/response schemas.
    Schema(ProviderSchemaArgs),
}

#[derive(Debug, Args)]
struct ProviderSchemaArgs {
    /// Provider capability whose wire contract should be printed.
    #[arg(value_enum)]
    capability: ProviderCapability,
    /// Emit compact JSON instead of pretty JSON.
    #[arg(long)]
    compact: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ProviderCapability {
    ContractGeneration,
    VisualGrounding,
}

pub(crate) fn execute(args: ProviderArgs) -> Result<ExitCode> {
    match args.command {
        ProviderCommand::Schema(args) => print_schema(args),
    }
}

fn print_schema(args: ProviderSchemaArgs) -> Result<ExitCode> {
    let schema = match args.capability {
        ProviderCapability::ContractGeneration => contract_generation_provider_schema(),
        ProviderCapability::VisualGrounding => visual_grounding_provider_schema(),
    };
    print_json(&schema, args.compact)?;
    Ok(ExitCode::SUCCESS)
}

fn print_json(schema: &ProviderProtocolSchema, compact: bool) -> Result<()> {
    if compact {
        println!("{}", serde_json::to_string(schema)?);
    } else {
        println!("{}", serde_json::to_string_pretty(schema)?);
    }
    Ok(())
}
