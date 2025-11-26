use structopt::StructOpt;

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    name = "deckbuilder-service",
    about = "Async Deckbuiler GraphQL service"
)]
pub struct Opt {
    /// Config file path
    #[structopt(short, long, default_value = "config.toml")]
    pub config: std::path::PathBuf,
}
