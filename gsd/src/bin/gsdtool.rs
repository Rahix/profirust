use console::style;
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
    /// Run the interactive configuration wizard.
    ConfigWizard(ConfigWizardOptions),
}

#[derive(Debug, Options)]
struct DumpOptions {
    help: bool,

    /// Path to the GSD file.
    #[options(free, required)]
    gsd_path: std::path::PathBuf,
}

#[derive(Debug, Options)]
struct ConfigWizardOptions {
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
        Some(GsdToolCommand::ConfigWizard(args)) => {
            run_config_wizard(&args);
        }
        None => panic!("No command specified"),
    }
}

fn run_config_wizard(args: &ConfigWizardOptions) {
    let gsd = gsd_parser::parse_from_file(&args.gsd_path);

    println!(
        "{}",
        style("Welcome to the station configuration wizard!").bold()
    );
    println!("Station: {:?} from {:?}", gsd.model, gsd.vendor);
    println!("Ident:   0x{:04x}", gsd.ident_number);
    println!();

    println!("{}", style("Global parameters:").bold());
    let mut prm = gsd_parser::PrmBuilder::new(&gsd);
    for (_, prm_ref) in gsd.user_prm_data.data_ref.iter() {
        if let Some(texts) = prm_ref.text_ref.as_ref() {
            let texts_list: Vec<_> = texts.keys().collect();
            let default = texts
                .values()
                .enumerate()
                .find(|(_, v)| **v == prm_ref.default_value)
                .unwrap()
                .0;
            dialoguer::Select::new()
                .with_prompt(&prm_ref.name)
                .items(&texts_list)
                .default(default)
                .interact()
                .unwrap();
        } else {
            todo!();
        }
    }
    println!();

    println!(
        "{}",
        style(format!("Selecting modules (maximum {}):", gsd.max_modules)).bold()
    );

    let module_names: Vec<String> = gsd
        .available_modules
        .iter()
        .map(|m| m.name.to_string())
        .collect();

    for i in 0..gsd.max_modules {
        let selection = dialoguer::FuzzySelect::new()
            .with_prompt(format!(
                "Select module {}/{} (ESC to stop)",
                i + 1,
                gsd.max_modules
            ))
            .items(&module_names)
            .interact_opt()
            .unwrap();

        if let Some(s) = selection {
            let module = gsd
                .available_modules
                .iter()
                .find(|m| m.name == module_names[s])
                .unwrap();

            for (_, prm_ref) in module.module_prm_data.data_ref.iter() {
                if let Some(texts) = prm_ref.text_ref.as_ref() {
                    let texts_list: Vec<_> = texts.keys().collect();
                    let default = texts
                        .values()
                        .enumerate()
                        .find(|(_, v)| **v == prm_ref.default_value)
                        .unwrap()
                        .0;
                    dialoguer::Select::new()
                        .with_prompt(&prm_ref.name)
                        .items(&texts_list)
                        .default(default)
                        .interact()
                        .unwrap();
                } else {
                    dialoguer::Input::new()
                        .with_prompt(format!(
                            "{} ({} - {})",
                            prm_ref.name, prm_ref.min_value, prm_ref.max_value
                        ))
                        .default(prm_ref.default_value.to_string())
                        .validate_with(|inp: &String| -> Result<(), &str> {
                            str::parse::<i64>(inp)
                                .ok()
                                .filter(|v| prm_ref.min_value <= *v && *v <= prm_ref.max_value)
                                .map(|_| ())
                                .ok_or("not a valid value")
                        })
                        .interact()
                        .unwrap();
                }
            }

            println!("Module Config: {:?}", module.config);
        } else {
            break;
        }
    }

    println!();
}
