use std::collections::BTreeMap;

mod gsd_parser {
    #[derive(pest_derive::Parser)]
    #[grammar = "gsd.pest"]
    pub struct GsdParser;
}

fn parse_number(pair: pest::iterators::Pair<'_, gsd_parser::Rule>) -> u32 {
    match pair.as_rule() {
        gsd_parser::Rule::dec_number => pair.as_str().parse().unwrap(),
        gsd_parser::Rule::hex_number => pair.as_str().parse().unwrap(),
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
                    _ => (),
                }
            }
            _ => (),
        }
    }

    gsd
}
