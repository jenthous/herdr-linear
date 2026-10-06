use clap::Parser;

fn main() {
    let cli = herdr_linear::cli::Cli::parse();
    match herdr_linear::cli::run(cli) {
        Ok(out) => {
            if !out.is_empty() {
                println!("{out}");
            }
        }
        Err(e) => {
            eprintln!("오류: {e:#}");
            std::process::exit(1);
        }
    }
}
