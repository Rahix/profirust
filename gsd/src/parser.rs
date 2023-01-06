use std::collections::BTreeMap;

mod gsd_parser {
    #[derive(pest_derive::Parser)]
    #[grammar = "gsd.pest"]
    pub struct GsdParser;
}

fn parse_number(pair: pest::iterators::Pair<'_, gsd_parser::Rule>) -> u32 {
    match pair.as_rule() {
        gsd_parser::Rule::dec_number => pair.as_str().parse().unwrap(),
        gsd_parser::Rule::hex_number => {
            u32::from_str_radix(pair.as_str().trim_start_matches("0x"), 16).unwrap()
        }
        _ => unreachable!("Called parse_number() on a non-number pair: {:?}", pair),
    }
}

fn parse_string_literal(pair: pest::iterators::Pair<'_, gsd_parser::Rule>) -> String {
    assert!(pair.as_rule() == gsd_parser::Rule::string_literal);
    // drop the quotation marks
    let mut chars = pair.as_str().chars();
    chars.next();
    chars.next_back();
    chars.as_str().to_owned()
}

pub fn parse(file: &std::path::Path, source: &str) -> crate::GenericStationDescription {
    use pest::Parser;

    let gsd_pairs = match gsd_parser::GsdParser::parse(gsd_parser::Rule::gsd, &source) {
        Ok(mut res) => res.next().unwrap(),
        Err(e) => panic!("{}", e.with_path(&file.to_string_lossy())),
    };

    let mut gsd = crate::GenericStationDescription::default();
    let mut prm_texts = BTreeMap::new();

    for statement in gsd_pairs.into_inner() {
        match statement.as_rule() {
            gsd_parser::Rule::prm_text => {
                let mut content = statement.into_inner();
                let id = parse_number(content.next().unwrap());
                let mut values = BTreeMap::new();
                for value_pairs in content {
                    assert!(value_pairs.as_rule() == gsd_parser::Rule::prm_text_value);
                    let mut iter = value_pairs.into_inner();
                    let number = parse_number(iter.next().unwrap());
                    let value = parse_string_literal(iter.next().unwrap());
                    assert!(iter.next().is_none());
                    values.insert(value, number);
                }
                prm_texts.insert(id, values);
            }
            gsd_parser::Rule::setting => {
                let mut pairs = statement.into_inner();
                let key = pairs.next().unwrap().as_str();
                let value_pair = pairs.next().unwrap();
                match key.to_lowercase().as_str() {
                    "gsd_revision" => gsd.gsd_revision = parse_number(value_pair) as u8,
                    "vendor_name" => gsd.vendor = parse_string_literal(value_pair),
                    "model_name" => gsd.model = parse_string_literal(value_pair),
                    "revision" => gsd.revision = parse_string_literal(value_pair),
                    "revision_number" => gsd.revision_number = parse_number(value_pair) as u8,
                    "ident_number" => gsd.ident_number = parse_number(value_pair) as u16,
                    //
                    "9.6_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B9600;
                        }
                    }
                    "19.2_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B19200;
                        }
                    }
                    "93.75_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B93750;
                        }
                    }
                    "187.5_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B187500;
                        }
                    }
                    "500_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B500000;
                        }
                    }
                    "1.5M_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B1500000;
                        }
                    }
                    "3M_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B3000000;
                        }
                    }
                    "6M_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B6000000;
                        }
                    }
                    "12M_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B12000000;
                        }
                    }
                    //
                    "modular_station" => gsd.modular_station = parse_number(value_pair) != 0,
                    _ => (),
                }
            }
            _ => (),
        }
    }

    gsd
}
