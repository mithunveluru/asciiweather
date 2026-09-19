use clap::Parser;

use asciiweather::cli::{self, Cli};

fn main() {
    let cli = Cli::parse();
    if let Err(err) = cli::run(&cli) {
        eprint!("{}", cli::report(&err, cli.debug));
        std::process::exit(err.exit_code());
    }
}
