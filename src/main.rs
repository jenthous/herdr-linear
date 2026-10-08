use herdr_linear::i18n::t;

fn main() {
    herdr_linear::cli::init_language();
    let cli = herdr_linear::cli::parse_args();
    match herdr_linear::cli::run(cli) {
        Ok(out) => {
            if !out.is_empty()
                && let Err(e) = herdr_linear::cli::write_out(&mut std::io::stdout().lock(), &out)
            {
                eprintln!("{}", (t().write_failed)(&e.to_string()));
                std::process::exit(1);
            }
        }
        Err(e) => {
            // 서버가 준 오류 문구가 섞일 수 있어 제어 문자를 지운다
            eprintln!(
                "{}",
                (t().error_line)(&herdr_linear::markdown::sanitize(&format!("{e:#}")))
            );
            std::process::exit(1);
        }
    }
}
