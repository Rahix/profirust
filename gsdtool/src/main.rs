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
            println!("{:#?}", gsd);
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
    let mut prm = gsd_parser::PrmBuilder::new(&gsd.user_prm_data);
    let mut global_parameters = vec![];
    for (_, prm_ref) in gsd.user_prm_data.data_ref.iter() {
        if let Some(texts) = prm_ref.text_ref.as_ref() {
            let texts_list: Vec<_> = texts.keys().collect();
            let default = texts
                .values()
                .enumerate()
                .find(|(_, v)| **v == prm_ref.default_value)
                .unwrap()
                .0;
            let selection = dialoguer::Select::new()
                .with_prompt(&prm_ref.name)
                .items(&texts_list)
                .default(default)
                .max_length(16)
                .interact()
                .unwrap();

            let sel_text = &texts_list[selection];
            prm.set_prm_from_text(&prm_ref.name, sel_text);

            global_parameters.push((prm_ref.name.to_owned(), sel_text.to_string()));
        } else {
            let value = dialoguer::Input::new()
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

            let value: i64 = str::parse(&value).unwrap();
            prm.set_prm(&prm_ref.name, value);

            global_parameters.push((prm_ref.name.to_owned(), value.to_string()));
        }
    }
    println!();

    let mut user_prm_data = Vec::new();
    user_prm_data.append(&mut prm.into_bytes());

    let mut module_config = Vec::new();

    println!(
        "{}",
        style(format!("Selecting modules (maximum {}):", gsd.max_modules)).bold()
    );

    let module_names: Vec<String> = gsd
        .available_modules
        .iter()
        .map(|m| m.name.to_string())
        .collect();

    let mut module_selection_list = vec![];
    for i in 0..gsd.max_modules {
        let selection = dialoguer::FuzzySelect::new()
            .with_prompt(format!(
                "Select module {}/{} (ESC to stop)",
                i + 1,
                gsd.max_modules
            ))
            .items(&module_names)
            .max_length(16)
            .interact_opt()
            .unwrap();

        if let Some(s) = selection {
            let module = gsd
                .available_modules
                .iter()
                .find(|m| m.name == module_names[s])
                .unwrap();

            module_config.append(&mut module.config.to_vec());

            let mut prm = gsd_parser::PrmBuilder::new(&module.module_prm_data);
            let mut module_parameters = vec![];
            for (_, prm_ref) in module.module_prm_data.data_ref.iter() {
                if let Some(texts) = prm_ref.text_ref.as_ref() {
                    let texts_list: Vec<_> = texts.keys().collect();
                    let default = texts
                        .values()
                        .enumerate()
                        .find(|(_, v)| **v == prm_ref.default_value)
                        .unwrap()
                        .0;
                    let selection = dialoguer::Select::new()
                        .with_prompt(&prm_ref.name)
                        .items(&texts_list)
                        .default(default)
                        .max_length(16)
                        .interact()
                        .unwrap();

                    let sel_text = &texts_list[selection];
                    prm.set_prm_from_text(&prm_ref.name, sel_text);

                    module_parameters.push((prm_ref.name.to_owned(), sel_text.to_string()));
                } else {
                    let value = dialoguer::Input::new()
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

                    let value: i64 = str::parse(&value).unwrap();
                    prm.set_prm(&prm_ref.name, value);

                    module_parameters.push((prm_ref.name.to_owned(), value.to_string()));
                }
            }

            module_selection_list.push((module_names[s].to_string(), module_parameters));

            user_prm_data.append(&mut prm.into_bytes());
        } else {
            break;
        }
    }
    println!();

    let mut bytes_input = 0;
    let mut bytes_output = 0;
    for cfg_byte in module_config.iter().copied() {
        let factor = if cfg_byte & 0x40 != 0 {
            // length in words
            2
        } else {
            // length in bytes
            1
        };
        let length = ((cfg_byte & 0x0f) + 1) * factor;
        if cfg_byte & 0x20 != 0 {
            bytes_output += length;
        }
        if cfg_byte & 0x10 != 0 {
            bytes_input += length;
        }
        if cfg_byte != 0 && cfg_byte & 0x30 == 0 {
            bytes_input = 0;
            bytes_output = 0;
            println!(
                "{}: Special module format not yet supported, I/O lengths are unknown.",
                style("Warning").yellow().bold()
            );
            break;
        }
    }

    println!();
    println!("{}", style("Peripheral Configuration:").bold());
    println!();
    println!(
        "    // Options generated by `gsdtool` using \"{}\"",
        args.gsd_path.file_name().unwrap().to_string_lossy()
    );
    println!("    let options = profirust::dp::PeripheralOptions {{");
    println!("        // \"{}\" by \"{}\"", gsd.model, gsd.vendor);
    println!("        ident_number: 0x{:04x},", gsd.ident_number);
    println!();
    println!("        // Global Parameters:");
    if global_parameters.len() == 0 {
        println!("        //   (none)");
    } else {
        let longest_name = global_parameters
            .iter()
            .map(|(n, _)| n.len())
            .max()
            .unwrap_or(0);
        for (name, value) in global_parameters.into_iter() {
            println!(
                "        //   - {:.<width$}: {}",
                name,
                value,
                width = longest_name
            );
        }
    }
    if module_selection_list.len() > 0 {
        println!("        //");
        println!("        // Selected Modules:");
        let modid_width = usize::try_from(module_selection_list.len().ilog10()).unwrap() + 1;
        for (i, (module, param)) in module_selection_list.into_iter().enumerate() {
            println!("        //   [{i:width$}] {}", module, width = modid_width);
            let longest_name = param.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
            for (name, value) in param.into_iter() {
                println!(
                    "        //    {:modid_width$}  - {:.<width$}: {}",
                    "",
                    name,
                    value,
                    width = longest_name,
                    modid_width = modid_width
                );
            }
        }
    }
    print!("        user_parameters: Some(&[");
    for b in user_prm_data.into_iter() {
        print!("0x{b:02x}, ");
    }
    println!("]),");
    print!("        config: Some(&[");
    for b in module_config.into_iter() {
        print!("0x{b:02x}, ");
    }
    println!("]),");
    println!();
    println!("        // Set max_tsdr depending on baudrate and assert");
    println!("        // that a supported baudrate is used.");
    println!("        max_tsdr: match BAUDRATE {{");
    for (_, speed) in gsd.supported_speeds.iter_names() {
        match speed {
            gsd_parser::SupportedSpeeds::B9600 => {
                println!(
                    "            profirust::Baudrate::B9600 => {},",
                    gsd.max_tsdr.b9600
                );
            }
            gsd_parser::SupportedSpeeds::B19200 => {
                println!(
                    "            profirust::Baudrate::B19200 => {},",
                    gsd.max_tsdr.b19200
                );
            }
            gsd_parser::SupportedSpeeds::B31250 => {
                println!(
                    "            profirust::Baudrate::B31250 => {},",
                    gsd.max_tsdr.b31250
                );
            }
            gsd_parser::SupportedSpeeds::B45450 => {
                println!(
                    "            profirust::Baudrate::B45450 => {},",
                    gsd.max_tsdr.b45450
                );
            }
            gsd_parser::SupportedSpeeds::B93750 => {
                println!(
                    "            profirust::Baudrate::B93750 => {},",
                    gsd.max_tsdr.b93750
                );
            }
            gsd_parser::SupportedSpeeds::B187500 => {
                println!(
                    "            profirust::Baudrate::B187500 => {},",
                    gsd.max_tsdr.b187500
                );
            }
            gsd_parser::SupportedSpeeds::B500000 => {
                println!(
                    "            profirust::Baudrate::B500000 => {},",
                    gsd.max_tsdr.b500000
                );
            }
            gsd_parser::SupportedSpeeds::B1500000 => {
                println!(
                    "            profirust::Baudrate::B1500000 => {},",
                    gsd.max_tsdr.b1500000
                );
            }
            gsd_parser::SupportedSpeeds::B3000000 => {
                println!(
                    "            profirust::Baudrate::B3000000 => {},",
                    gsd.max_tsdr.b3000000
                );
            }
            gsd_parser::SupportedSpeeds::B6000000 => {
                println!(
                    "            profirust::Baudrate::B6000000 => {},",
                    gsd.max_tsdr.b6000000
                );
            }
            gsd_parser::SupportedSpeeds::B12000000 => {
                println!(
                    "            profirust::Baudrate::B12000000 => {},",
                    gsd.max_tsdr.b12000000
                );
            }
            _ => unreachable!(),
        }
    }
    println!(
        "            b => panic!(\"Peripheral \\\"{}\\\" does not support baudrate {{b:?}}!\"),",
        gsd.model
    );
    println!("        }},");
    println!();
    println!("        ..Default::default()");
    println!("    }};");
    if bytes_input != 0 || bytes_output != 0 {
        println!("    let mut buffer_inputs = [0u8; {}];", bytes_input);
        println!("    let mut buffer_outputs = [0u8; {}];", bytes_output);
    }
    println!();
}
