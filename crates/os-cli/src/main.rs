use clap::Parser;

fn main() {
    let cli = os_cli::Cli::parse();
    std::process::exit(os_cli::run(cli));
}
