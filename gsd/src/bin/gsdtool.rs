use gumdrop::Options;

#[derive(Debug, Options)]
struct GsdToolOptions {
    help: bool,

    #[options(command)]
    command: Option<GsdToolCommand>,
}

#[derive(Debug, Options)]
enum GsdToolCommand {
    /// Dump the contents of the GSD file as a Rust structure.
    Dump(DumpOptions),
}

#[derive(Debug, Options)]
struct DumpOptions {
    help: bool,

    /// Path to the GSD file.
    #[options(free, required)]
    gsd_path: std::path::PathBuf,
}

fn main() {
    let args = GsdToolOptions::parse_args_default_or_exit();
    match args.command {
        Some(GsdToolCommand::Dump(args)) => {
            let gsd = gsd_parser::parse_from_file(args.gsd_path);
            dbg!(gsd);
        }
        None => panic!("No command specified"),
    }
}
