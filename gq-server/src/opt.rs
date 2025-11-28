use clio::Input;

#[derive(Clone, Debug, clap::Parser)]
#[command(
    name = "deckbuilder-service",
    about = "Async Deckbuiler GraphQL service"
)]
pub struct Opt {
    /// Config file path
    #[structopt(short, long, value_parser, default_value = "config.toml")]
    pub config: Input,
}
