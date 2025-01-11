use std::collections::BTreeMap;
use std::sync::Arc;

mod gsd_parser {
    #[derive(pest_derive::Parser)]
    #[grammar = "gsd.pest"]
    pub struct GsdParser;
}

pub type ParseError = pest::error::Error<gsd_parser::Rule>;
pub type ParseResult<T> = Result<T, ParseError>;

fn parse_error(e: impl std::fmt::Display, span: pest::Span<'_>) -> ParseError {
    let message = format!("{}", e);
    pest::error::Error::new_from_span(pest::error::ErrorVariant::CustomError { message }, span)
}

fn parse_number<T: TryFrom<u32>>(
    pair: pest::iterators::Pair<'_, gsd_parser::Rule>,
) -> ParseResult<T>
where
    <T as TryFrom<u32>>::Error: std::fmt::Display,
{
    match pair.as_rule() {
        gsd_parser::Rule::dec_number => pair.as_str().parse(),
        gsd_parser::Rule::hex_number => {
            u32::from_str_radix(pair.as_str().trim_start_matches("0x"), 16)
        }
        _ => panic!("Called parse_number() on a non-number pair: {:?}", pair),
    }
    .map_err(|_| parse_error("invalid digit found while parsing integer", pair.as_span()))
    .and_then(|i| i.try_into().map_err(|e| parse_error(e, pair.as_span())))
}

fn parse_signed_number(pair: pest::iterators::Pair<'_, gsd_parser::Rule>) -> ParseResult<i64> {
    match pair.as_rule() {
        gsd_parser::Rule::dec_number => pair.as_str().parse(),
        gsd_parser::Rule::hex_number => {
            i64::from_str_radix(pair.as_str().trim_start_matches("0x"), 16)
        }
        _ => panic!("Called parse_number() on a non-number pair: {:?}", pair),
    }
    .map_err(|_| {
        parse_error(
            "invalid digit found while parsing signed integer",
            pair.as_span(),
        )
    })
}

fn parse_number_list<T: TryFrom<u32>>(
    pair: pest::iterators::Pair<'_, gsd_parser::Rule>,
) -> ParseResult<Vec<T>>
where
    <T as TryFrom<u32>>::Error: std::fmt::Display,
{
    Ok(match pair.as_rule() {
        gsd_parser::Rule::number_list => pair
            .into_inner()
            .into_iter()
            .map(|p| parse_number::<T>(p))
            .collect::<ParseResult<Vec<T>>>()?,
        gsd_parser::Rule::dec_number | gsd_parser::Rule::hex_number => {
            vec![parse_number(pair)?]
        }
        _ => panic!(
            "Called parse_number_list() on a pair that cannot be a number list: {:?}",
            pair
        ),
    })
}

fn parse_bool(pair: pest::iterators::Pair<'_, gsd_parser::Rule>) -> ParseResult<bool> {
    Ok(parse_number::<u32>(pair)? != 0)
}

fn parse_string_literal(pair: pest::iterators::Pair<'_, gsd_parser::Rule>) -> String {
    assert!(pair.as_rule() == gsd_parser::Rule::string_literal);
    // drop the quotation marks
    let mut chars = pair.as_str().chars();
    chars.next();
    chars.next_back();
    chars.as_str().to_owned()
}

pub fn parse(
    file: &std::path::Path,
    source: &str,
) -> ParseResult<crate::GenericStationDescription> {
    parse_inner(source).map_err(|e| e.with_path(&file.to_string_lossy()))
}

fn parse_inner(source: &str) -> ParseResult<crate::GenericStationDescription> {
    use pest::Parser;

    let gsd_pairs = gsd_parser::GsdParser::parse(gsd_parser::Rule::gsd, &source)?
        .next()
        .expect("pest grammar wrong?");

    let mut gsd = crate::GenericStationDescription::default();
    let mut prm_texts = BTreeMap::new();
    let mut user_prm_data_definitions = BTreeMap::new();
    let mut legacy_prm = Some(crate::UserPrmData::default());

    for statement in gsd_pairs.into_inner() {
        let statement_span = statement.as_span();
        match statement.as_rule() {
            gsd_parser::Rule::prm_text => {
                let mut content = statement.into_inner();
                let id: u16 = parse_number(content.next().expect("pest grammar wrong?"))?;
                let mut values = BTreeMap::new();
                for value_pairs in content {
                    assert!(value_pairs.as_rule() == gsd_parser::Rule::prm_text_value);
                    let mut iter = value_pairs.into_inner();
                    let number = parse_signed_number(iter.next().expect("pest grammar wrong?"))?;
                    let value = parse_string_literal(iter.next().expect("pest grammar wrong?"));
                    assert!(iter.next().is_none());
                    values.insert(value, number);
                }
                prm_texts.insert(id, Arc::new(values));
            }
            gsd_parser::Rule::ext_user_prm_data => {
                let mut content = statement.into_inner();
                // TODO: actually u32?
                let id: u32 = parse_number(content.next().expect("pest grammar wrong?"))?;
                let name = parse_string_literal(content.next().expect("pest grammar wrong?"));

                let data_type_pair = content.next().expect("pest grammar wrong?");
                assert_eq!(
                    data_type_pair.as_rule(),
                    gsd_parser::Rule::prm_data_type_name
                );
                let data_type_rule = data_type_pair
                    .into_inner()
                    .next()
                    .expect("pest grammar wrong?");
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
                        let bit = parse_number(
                            data_type_rule
                                .into_inner()
                                .next()
                                .expect("pest grammar wrong?"),
                        )?;
                        crate::UserPrmDataType::Bit(bit)
                    }
                    gsd_parser::Rule::bit_area => {
                        let mut content = data_type_rule.into_inner();
                        let first_bit = parse_number(content.next().expect("pest grammar wrong?"))?;
                        let last_bit = parse_number(content.next().expect("pest grammar wrong?"))?;
                        crate::UserPrmDataType::BitArea(first_bit, last_bit)
                    }
                    _ => unreachable!(),
                };

                let default_value =
                    parse_signed_number(content.next().expect("pest grammar wrong?"))?;

                let mut constraint = crate::PrmValueConstraint::Unconstrained;
                let mut text_ref = None;
                let mut changeable = true;
                let mut visible = true;

                for rule in content {
                    match rule.as_rule() {
                        gsd_parser::Rule::prm_data_value_range => {
                            let mut content = rule.into_inner();
                            let min_value =
                                parse_signed_number(content.next().expect("pest grammar wrong?"))?;
                            let max_value =
                                parse_signed_number(content.next().expect("pest grammar wrong?"))?;
                            constraint = crate::PrmValueConstraint::MinMax(min_value, max_value);
                        }
                        gsd_parser::Rule::prm_data_value_set => {
                            let mut values = Vec::new();
                            for pairs in rule.into_inner() {
                                let number = parse_signed_number(pairs)?;
                                values.push(number);
                            }
                            constraint = crate::PrmValueConstraint::Enum(values);
                        }
                        gsd_parser::Rule::prm_text_ref => {
                            let text_id = parse_number(
                                rule.into_inner().next().expect("pest grammar wrong?"),
                            )?;
                            text_ref = Some(
                                prm_texts
                                    .get(&text_id)
                                    .ok_or_else(|| {
                                        parse_error(
                                            format!("PrmText {} was not found", text_id),
                                            statement_span,
                                        )
                                    })?
                                    .clone(),
                            );
                        }
                        gsd_parser::Rule::prm_data_changeable => {
                            changeable = parse_bool(rule.into_inner().next().unwrap())?;
                        }
                        gsd_parser::Rule::prm_data_visible => {
                            visible = parse_bool(rule.into_inner().next().unwrap())?;
                        }
                        rule => unreachable!("unexpected rule {rule:?}"),
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
                        changeable,
                        visible,
                    }),
                );
            }
            gsd_parser::Rule::unit_diag_area => {
                let mut content = statement.into_inner();
                let first = parse_number(content.next().unwrap())?;
                let last = parse_number(content.next().unwrap())?;
                let mut values = BTreeMap::new();
                for value_pairs in content {
                    assert!(value_pairs.as_rule() == gsd_parser::Rule::unit_diag_area_value);
                    let mut iter = value_pairs.into_inner();
                    let number = parse_number(iter.next().unwrap())?;
                    let value = parse_string_literal(iter.next().unwrap());
                    assert!(iter.next().is_none());
                    values.insert(number, value);
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
                let module_config: Vec<u8> = parse_number_list(content.next().unwrap())?;
                let mut module_reference = None;
                let mut module_prm_data = crate::UserPrmData::default();

                for rule in content {
                    match rule.as_rule() {
                        gsd_parser::Rule::module_reference => {
                            module_reference =
                                Some(parse_number(rule.into_inner().next().unwrap())?);
                        }
                        gsd_parser::Rule::setting => {
                            let mut pairs = rule.into_inner();
                            let key = pairs.next().unwrap().as_str();
                            let value_pair = pairs.next().unwrap();
                            match key.to_lowercase().as_str() {
                                "ext_module_prm_data_len" => {
                                    module_prm_data.length = parse_number(value_pair)?;
                                }
                                "ext_user_prm_data_ref" => {
                                    let offset = parse_number(value_pair)?;
                                    let data_id = parse_number(pairs.next().unwrap())?;
                                    let data_ref =
                                        user_prm_data_definitions.get(&data_id).unwrap().clone();
                                    module_prm_data.data_ref.push((offset, data_ref));
                                }
                                "ext_user_prm_data_const" => {
                                    let offset = parse_number(value_pair)?;
                                    let values: Vec<u8> = parse_number_list(pairs.next().unwrap())?;
                                    module_prm_data.data_const.push((offset, values));
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
                gsd.available_modules.push(Arc::new(module));
            }
            gsd_parser::Rule::slot_definition => {
                for rule in statement.into_inner() {
                    match rule.as_rule() {
                        gsd_parser::Rule::slot => {
                            let mut pairs = rule.into_inner();
                            let number = parse_number(pairs.next().unwrap())?;
                            let name = parse_string_literal(pairs.next().unwrap());

                            #[allow(unused)]
                            let find_module =
                                |reference: u16,
                                 slot_ref: &str,
                                 slot_num: u8|
                                 -> Option<Arc<crate::Module>> {
                                    for module in gsd.available_modules.iter() {
                                        if module.reference == Some(reference.into()) {
                                            return Some(module.clone());
                                        }
                                    }
                                    // TODO: Warning management?
                                    // log::warn!("No module with reference {reference} found for slot {slot_num} (\"{slot_ref}\")");
                                    None
                                };

                            let default_pair = pairs.next().unwrap();
                            let default_span = default_pair.as_span();
                            let default_ref = parse_number(default_pair)?;

                            let value_pair = pairs.next().unwrap();
                            let allowed_modules = match value_pair.as_rule() {
                                gsd_parser::Rule::slot_value_range => {
                                    let mut pairs = value_pair.into_inner();
                                    let first = parse_number(pairs.next().unwrap())?;
                                    let last = parse_number(pairs.next().unwrap())?;
                                    (first..=last)
                                        .filter_map(|r| find_module(r, &name, number))
                                        .collect::<Vec<_>>()
                                }
                                gsd_parser::Rule::slot_value_set => {
                                    let mut allowed_modules = Vec::new();
                                    for pairs in value_pair.into_inner() {
                                        let reference = parse_number(pairs)?;
                                        if let Some(module) = find_module(reference, &name, number)
                                        {
                                            allowed_modules.push(module);
                                        }
                                    }
                                    allowed_modules
                                }
                                r => unreachable!("found rule {r:?}"),
                            };

                            let Some(default) = find_module(default_ref, &name, number) else {
                                return Err(parse_error(
                                    format!(
                                        "The default module for slot {number} (\"{name}\") with reference {default_ref} is not available",
                                    ),
                                    default_span,
                                ));
                            };
                            if !allowed_modules.contains(&default) {
                                // TODO: Warning management?
                                // log::warn!("Default module not part of allowed modules?!");
                            }

                            let slot = crate::Slot {
                                name,
                                number,
                                default,
                                allowed_modules,
                            };

                            gsd.slots.push(slot);
                        }
                        r => unreachable!("found rule {r:?}"),
                    }
                }
            }
            gsd_parser::Rule::setting => {
                let mut pairs = statement.into_inner();
                let key = pairs.next().unwrap().as_str();
                let value_pair = pairs.next().unwrap();
                match key.to_lowercase().as_str() {
                    "gsd_revision" => gsd.gsd_revision = parse_number(value_pair)?,
                    "vendor_name" => gsd.vendor = parse_string_literal(value_pair),
                    "model_name" => gsd.model = parse_string_literal(value_pair),
                    "revision" => gsd.revision = parse_string_literal(value_pair),
                    "revision_number" => gsd.revision_number = parse_number(value_pair)?,
                    "ident_number" => gsd.ident_number = parse_number(value_pair)?,
                    //
                    "hardware_release" => gsd.hardware_release = parse_string_literal(value_pair),
                    "software_release" => gsd.software_release = parse_string_literal(value_pair),
                    //
                    "fail_safe" => gsd.fail_safe = parse_bool(value_pair)?,
                    //
                    "9.6_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B9600;
                        }
                    }
                    "19.2_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B19200;
                        }
                    }
                    "31.25_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B31250;
                        }
                    }
                    "45.45_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B45450;
                        }
                    }
                    "93.75_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B93750;
                        }
                    }
                    "187.5_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B187500;
                        }
                    }
                    "500_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B500000;
                        }
                    }
                    "1.5m_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B1500000;
                        }
                    }
                    "3m_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B3000000;
                        }
                    }
                    "6m_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B6000000;
                        }
                    }
                    "12m_supp" => {
                        if parse_bool(value_pair)? {
                            gsd.supported_speeds |= crate::SupportedSpeeds::B12000000;
                        }
                    }
                    "maxtsdr_9.6" => gsd.max_tsdr.b9600 = parse_number(value_pair)?,
                    "maxtsdr_19.2" => gsd.max_tsdr.b19200 = parse_number(value_pair)?,
                    "maxtsdr_31.25" => gsd.max_tsdr.b31250 = parse_number(value_pair)?,
                    "maxtsdr_45.45" => gsd.max_tsdr.b45450 = parse_number(value_pair)?,
                    "maxtsdr_93.75" => gsd.max_tsdr.b93750 = parse_number(value_pair)?,
                    "maxtsdr_187.5" => gsd.max_tsdr.b187500 = parse_number(value_pair)?,
                    "maxtsdr_500" => gsd.max_tsdr.b500000 = parse_number(value_pair)?,
                    "maxtsdr_1.5m" => gsd.max_tsdr.b1500000 = parse_number(value_pair)?,
                    "maxtsdr_3m" => gsd.max_tsdr.b3000000 = parse_number(value_pair)?,
                    "maxtsdr_6m" => gsd.max_tsdr.b6000000 = parse_number(value_pair)?,
                    "maxtsdr_12m" => gsd.max_tsdr.b12000000 = parse_number(value_pair)?,
                    "implementation_type" => {
                        gsd.implementation_type = parse_string_literal(value_pair)
                    }
                    //
                    "modular_station" => gsd.modular_station = parse_bool(value_pair)?,
                    "max_module" => gsd.max_modules = parse_number(value_pair)?,
                    "max_input_len" => gsd.max_input_length = parse_number(value_pair)?,
                    "max_output_len" => gsd.max_output_length = parse_number(value_pair)?,
                    "max_data_len" => gsd.max_data_length = parse_number(value_pair)?,
                    "max_diag_data_len" => gsd.max_diag_data_length = parse_number(value_pair)?,
                    "freeze_mode_supp" => gsd.freeze_mode_supported = parse_bool(value_pair)?,
                    "sync_mode_supp" => gsd.sync_mode_supported = parse_bool(value_pair)?,
                    "auto_baud_supp" => gsd.auto_baud_supported = parse_bool(value_pair)?,
                    "set_slave_add_supp" => gsd.set_slave_addr_supported = parse_bool(value_pair)?,
                    "ext_user_prm_data_ref" => {
                        let offset = parse_number(value_pair)?;
                        let data_id = parse_number(pairs.next().unwrap())?;
                        let data_ref = user_prm_data_definitions.get(&data_id).unwrap().clone();
                        gsd.user_prm_data.data_ref.push((offset, data_ref));
                        // The presence of this keywords means `User_Prm_Data` and
                        // `User_Prm_Data_Len` should be ignored.
                        legacy_prm = None;
                    }
                    "ext_user_prm_data_const" => {
                        let offset = parse_number(value_pair)?;
                        let values: Vec<u8> = parse_number_list(pairs.next().unwrap())?;
                        gsd.user_prm_data.data_const.push((offset, values));
                        // The presence of this keywords means `User_Prm_Data` and
                        // `User_Prm_Data_Len` should be ignored.
                        legacy_prm = None;
                    }
                    "max_user_prm_data_len" => {
                        // TODO: Actually evaluate this value.

                        // The presence of this keywords means `User_Prm_Data` and
                        // `User_Prm_Data_Len` should be ignored.
                        legacy_prm = None;
                    }
                    "user_prm_data_len" => {
                        // If legacy_prm is not None, we didn't encounter new-style Ext_User_Prm
                        // yet, so legacy User_Prm_Data should be evaluated.
                        if let Some(prm) = legacy_prm.as_mut() {
                            prm.length = parse_number(value_pair)?;

                            // Check if length matches data
                            let current_max_length = prm
                                .data_const
                                .iter()
                                .map(|(offset, values)| offset + values.len())
                                .max()
                                .unwrap_or(0);

                            if usize::from(prm.length) < current_max_length {
                                return Err(parse_error(
                                    format!(
                                        "User_Prm_Data has maximum length of {} while User_Prm_Data_Len only permits {}",
                                        current_max_length,
                                        prm.length,
                                    ),
                                    statement_span,
                                ));
                            }
                        }
                    }
                    "user_prm_data" => {
                        // If legacy_prm is not None, we didn't encounter new-style Ext_User_Prm
                        // yet, so legacy User_Prm_Data should be evaluated.
                        if let Some(prm) = legacy_prm.as_mut() {
                            let values: Vec<u8> = parse_number_list(value_pair)?;

                            // Only check length when it was already defined
                            if prm.length != 0 && usize::from(prm.length) < values.len() {
                                return Err(parse_error(
                                    format!(
                                        "User_Prm_Data has maximum length of {} while User_Prm_Data_Len only permits {}",
                                        values.len(),
                                        prm.length,
                                    ),
                                    statement_span,
                                ));
                            }

                            prm.data_const.push((0, values));
                        }
                    }
                    "unit_diag_bit" => {
                        let bit = parse_number(value_pair)?;
                        let text = parse_string_literal(pairs.next().unwrap());
                        gsd.unit_diag.bits.entry(bit).or_default().text = text;
                    }
                    "unit_diag_bit_help" => {
                        let bit = parse_number(value_pair)?;
                        let text = parse_string_literal(pairs.next().unwrap());
                        gsd.unit_diag.bits.entry(bit).or_default().help = Some(text);
                    }
                    "unit_diag_not_bit" => {
                        let bit = parse_number(value_pair)?;
                        let text = parse_string_literal(pairs.next().unwrap());
                        gsd.unit_diag.not_bits.entry(bit).or_default().text = text;
                    }
                    "unit_diag_not_bit_help" => {
                        let bit = parse_number(value_pair)?;
                        let text = parse_string_literal(pairs.next().unwrap());
                        gsd.unit_diag.not_bits.entry(bit).or_default().help = Some(text);
                    }
                    _ => (),
                }
            }
            _ => (),
        }
    }

    // If no `Ext_User_Prm` was present, commit the legacy Prm data into the gsd struct.
    if let Some(prm) = legacy_prm {
        gsd.user_prm_data = prm;
    }

    // If this is a compact station, only allow one module
    if !gsd.modular_station {
        if !gsd.max_modules == 1 {
            // TODO: Warnings
        }
        if !gsd.available_modules.len() == 1 {
            // TODO: Warnings
        }
        gsd.max_modules = 1;
    }

    Ok(gsd)
}
