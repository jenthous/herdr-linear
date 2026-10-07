use clap::Parser;

fn main() {
    let cli = herdr_linear::cli::Cli::parse();
    match herdr_linear::cli::run(cli) {
        Ok(out) => {
            if !out.is_empty()
                && let Err(e) = herdr_linear::cli::write_out(&mut std::io::stdout().lock(), &out)
            {
                eprintln!("오류: 출력하지 못했어요: {e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            // 서버가 준 오류 문구가 섞일 수 있어 제어 문자를 지운다
            eprintln!(
                "오류: {}",
                herdr_linear::markdown::sanitize(&format!("{e:#}"))
            );
            std::process::exit(1);
        }
    }
}
