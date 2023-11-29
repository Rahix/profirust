use std::collections::BTreeMap;
use std::sync::Arc;

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

fn parse_number_list<T: TryFrom<u32>>(pair: pest::iterators::Pair<'_, gsd_parser::Rule>) -> Vec<T> {
    match pair.as_rule() {
        gsd_parser::Rule::number_list => pair
            .into_inner()
            .into_iter()
            .map(|p| parse_number(p).try_into().ok().unwrap())
            .collect(),
        gsd_parser::Rule::dec_number | gsd_parser::Rule::hex_number => {
            vec![parse_number(pair).try_into().ok().unwrap()]
        }
        _ => unreachable!(),
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
    let mut user_prm_data_definitions = BTreeMap::new();

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
                    values.insert(value, number as i64);
                }
                prm_texts.insert(id, Arc::new(values));
            }
            gsd_parser::Rule::ext_user_prm_data => {
                let mut content = statement.into_inner();
                let id = parse_number(content.next().unwrap());
                let name = parse_string_literal(content.next().unwrap());

                let data_type_pair = content.next().unwrap();
                assert_eq!(
                    data_type_pair.as_rule(),
                    gsd_parser::Rule::prm_data_type_name
                );
                let data_type_rule = data_type_pair.into_inner().next().unwrap();
                let data_type = match data_type_rule.as_rule() {
                    gsd_parser::Rule::identifier => {
                        match data_type_rule.as_str().to_lowercase().as_str() {
                            "unsigned8" => crate::UserPrmDataType::Unsigned8,
                            "unsigned16" => crate::UserPrmDataType::Unsigned16,
                            "unsigned32" => crate::UserPrmDataType::Unsigned32,
                            "signed8" => crate::UserPrmDataType::Signed8,
                            "signed16" => crate::UserPrmDataType::Signed16,
                            "signed32" => crate::UserPrmDataType::Signed32,
                            dt => panic!("unknown data type {dt:?}"),
                        }
                    }
                    gsd_parser::Rule::bit => {
                        let bit = parse_number(data_type_rule.into_inner().next().unwrap());
                        crate::UserPrmDataType::Bit(bit as u8)
                    }
                    gsd_parser::Rule::bit_area => {
                        let mut content = data_type_rule.into_inner();
                        let first_bit = parse_number(content.next().unwrap());
                        let last_bit = parse_number(content.next().unwrap());
                        crate::UserPrmDataType::BitArea(first_bit as u8, last_bit as u8)
                    }
                    _ => unreachable!(),
                };

                let default_value = parse_number(content.next().unwrap()) as i64;

                let value_constraint_rule = content.next().unwrap();
                let constraint = match value_constraint_rule.as_rule() {
                    gsd_parser::Rule::prm_data_value_range => {
                        let mut content = value_constraint_rule.into_inner();
                        let min_value = parse_number(content.next().unwrap()) as i64;
                        let max_value = parse_number(content.next().unwrap()) as i64;
                        crate::PrmValueConstraint::MinMax(min_value, max_value)
                    }
                    gsd_parser::Rule::prm_data_value_set => {
                        let mut values = Vec::new();
                        for pairs in value_constraint_rule.into_inner() {
                            let number = parse_number(pairs);
                            values.push(number as i64);
                        }
                        crate::PrmValueConstraint::Enum(values)
                    }
                    _ => unreachable!(),
                };

                let mut text_ref = None;
                for data_setting in content {
                    match data_setting.as_rule() {
                        gsd_parser::Rule::prm_text_ref => {
                            let text_id = parse_number(data_setting.into_inner().next().unwrap());
                            text_ref = Some(prm_texts.get(&text_id).unwrap().clone());
                        }
                        rule => todo!("rule {rule:?}"),
                    }
                }

                user_prm_data_definitions.insert(
                    id,
                    Arc::new(crate::UserPrmDataDefinition {
                        name,
                        data_type,
                        text_ref,
                        default_value,
                        constraint,
                    }),
                );
            }
            gsd_parser::Rule::unit_diag_area => {
                let mut content = statement.into_inner();
                let first = parse_number(content.next().unwrap()) as u16;
                let last = parse_number(content.next().unwrap()) as u16;
                let mut values = BTreeMap::new();
                for value_pairs in content {
                    assert!(value_pairs.as_rule() == gsd_parser::Rule::unit_diag_area_value);
                    let mut iter = value_pairs.into_inner();
                    let number = parse_number(iter.next().unwrap());
                    let value = parse_string_literal(iter.next().unwrap());
                    assert!(iter.next().is_none());
                    values.insert(number as u16, value);
                }
                gsd.unit_diag.areas.push(crate::UnitDiagArea {
                    first,
                    last,
                    values,
                });
            }
            gsd_parser::Rule::module => {
                let mut content = statement.into_inner();
                let name = parse_string_literal(content.next().unwrap());
                let module_config: Vec<u8> = parse_number_list(content.next().unwrap());
                let mut module_reference = None;
                let mut module_prm_data = crate::UserPrmData::default();

                for rule in content {
                    match rule.as_rule() {
                        gsd_parser::Rule::module_reference => {
                            module_reference =
                                Some(parse_number(rule.into_inner().next().unwrap()));
                        }
                        gsd_parser::Rule::setting => {
                            let mut pairs = rule.into_inner();
                            let key = pairs.next().unwrap().as_str();
                            let value_pair = pairs.next().unwrap();
                            match key.to_lowercase().as_str() {
                                "ext_module_prm_data_len" => {
                                    module_prm_data.length = parse_number(value_pair) as u8;
                                }
                                "ext_user_prm_data_ref" => {
                                    let offset = parse_number(value_pair);
                                    let data_id = parse_number(pairs.next().unwrap());
                                    let data_ref =
                                        user_prm_data_definitions.get(&data_id).unwrap().clone();
                                    module_prm_data.data_ref.push((offset as usize, data_ref));
                                }
                                "ext_user_prm_data_const" => {
                                    let offset = parse_number(value_pair);
                                    let values: Vec<u8> = parse_number_list(pairs.next().unwrap());
                                    module_prm_data.data_const.push((offset as usize, values));
                                }
                                _ => (),
                            }
                        }
                        gsd_parser::Rule::data_area => (),
                        r => unreachable!("found rule {r:?}"),
                    }
                }

                let module = crate::Module {
                    name,
                    config: module_config,
                    reference: module_reference,
                    module_prm_data,
                };
                gsd.available_modules.push(module);
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
                    "hardware_release" => gsd.hardware_release = parse_string_literal(value_pair),
                    "software_release" => gsd.software_release = parse_string_literal(value_pair),
                    //
                    "fail_safe" => gsd.fail_safe = parse_number(value_pair) != 0,
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
                    "31.25_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B31250;
                        }
                    }
                    "45.45_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B45450;
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
                    "1.5m_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B1500000;
                        }
                    }
                    "3m_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B3000000;
                        }
                    }
                    "6m_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B6000000;
                        }
                    }
                    "12m_supp" => {
                        if parse_number(value_pair) != 0 {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B12000000;
                        }
                    }
                    "maxtsdr_9.6" => gsd.max_tsdr.b9600 = parse_number(value_pair) as u16,
                    "maxtsdr_19.2" => gsd.max_tsdr.b19200 = parse_number(value_pair) as u16,
                    "maxtsdr_31.25" => gsd.max_tsdr.b31250 = parse_number(value_pair) as u16,
                    "maxtsdr_45.45" => gsd.max_tsdr.b45450 = parse_number(value_pair) as u16,
                    "maxtsdr_93.75" => gsd.max_tsdr.b93750 = parse_number(value_pair) as u16,
                    "maxtsdr_187.5" => gsd.max_tsdr.b187500 = parse_number(value_pair) as u16,
                    "maxtsdr_500" => gsd.max_tsdr.b500000 = parse_number(value_pair) as u16,
                    "maxtsdr_1.5m" => gsd.max_tsdr.b1500000 = parse_number(value_pair) as u16,
                    "maxtsdr_3m" => gsd.max_tsdr.b3000000 = parse_number(value_pair) as u16,
                    "maxtsdr_6m" => gsd.max_tsdr.b6000000 = parse_number(value_pair) as u16,
                    "maxtsdr_12m" => gsd.max_tsdr.b12000000 = parse_number(value_pair) as u16,
                    "implementation_type" => {
                        gsd.implementation_type = parse_string_literal(value_pair)
                    }
                    //
                    "modular_station" => gsd.modular_station = parse_number(value_pair) != 0,
                    "max_module" => gsd.max_modules = parse_number(value_pair) as u8,
                    "max_input_len" => gsd.max_input_length = parse_number(value_pair) as u8,
                    "max_output_len" => gsd.max_output_length = parse_number(value_pair) as u8,
                    "max_data_len" => gsd.max_data_length = parse_number(value_pair) as u8,
                    "max_diag_data_len" => {
                        gsd.max_diag_data_length = parse_number(value_pair) as u8
                    }
                    "freeze_mode_supp" => gsd.freeze_mode_supported = parse_number(value_pair) != 0,
                    "sync_mode_supp" => gsd.sync_mode_supported = parse_number(value_pair) != 0,
                    "auto_baud_supp" => gsd.auto_baud_supported = parse_number(value_pair) != 0,
                    "set_slave_add_supp" => {
                        gsd.set_slave_addr_supported = parse_number(value_pair) != 0
                    }
                    "ext_user_prm_data_ref" => {
                        let offset = parse_number(value_pair);
                        let data_id = parse_number(pairs.next().unwrap());
                        let data_ref = user_prm_data_definitions.get(&data_id).unwrap().clone();
                        gsd.user_prm_data.data_ref.push((offset as usize, data_ref));
                    }
                    "ext_user_prm_data_const" => {
                        let offset = parse_number(value_pair);
                        let values: Vec<u8> = parse_number_list(pairs.next().unwrap());
                        gsd.user_prm_data.data_const.push((offset as usize, values));
                    }
                    "unit_diag_bit" => {
                        let bit = parse_number(value_pair);
                        let text = parse_string_literal(pairs.next().unwrap());
                        gsd.unit_diag.bits.entry(bit).or_default().text = text;
                    }
                    "unit_diag_bit_help" => {
                        let bit = parse_number(value_pair);
                        let text = parse_string_literal(pairs.next().unwrap());
                        gsd.unit_diag.bits.entry(bit).or_default().help = Some(text);
                    }
                    "unit_diag_not_bit" => {
                        let bit = parse_number(value_pair);
                        let text = parse_string_literal(pairs.next().unwrap());
                        gsd.unit_diag.not_bits.entry(bit).or_default().text = text;
                    }
                    "unit_diag_not_bit_help" => {
                        let bit = parse_number(value_pair);
                        let text = parse_string_literal(pairs.next().unwrap());
                        gsd.unit_diag.not_bits.entry(bit).or_default().help = Some(text);
                    }
                    _ => (),
                }
            }
            _ => (),
        }
    }

    gsd
}
