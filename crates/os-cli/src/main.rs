// Windows gibt dem Hauptthread nur 1 MB Stack, Linux und macOS 8 MB. `--run-task` mit automatischem
// Vergleich (C02) läuft unter Windows über; deshalb läuft der Einstieg plattformgleich in einem Thread mit 8 MB.
const MAIN_STACK_SIZE: usize = 8 * 1024 * 1024;

fn main() {
    let code = std::thread::Builder::new()
        .name("main".into())
        .stack_size(MAIN_STACK_SIZE)
        .spawn(|| os_cli::main_entry(std::env::args_os()))
        .map(|handle| {
            handle
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
        .unwrap_or_else(|_| os_cli::main_entry(std::env::args_os()));
    std::process::exit(code);
}
