mod gsd_parser {
    #[derive(pest_derive::Parser)]
    #[grammar = "gsd.pest"]
    pub struct GsdParser;
}

pub fn parse(file: &std::path::Path, source: &str) -> crate::GenericStationDescription {
    use pest::Parser;

    let res = match gsd_parser::GsdParser::parse(gsd_parser::Rule::gsd, &source) {
        Ok(mut res) => res.next().unwrap(),
        Err(e) => panic!("{}", e.with_path(&file.to_string_lossy())),
    };

    for f in res.into_inner() {
        dbg!(f);
    }

    todo!()
}
