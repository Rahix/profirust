#[test]
fn parse_invalid_prm_text1() {
    let source = r#"
#Profibus_DP
PrmText = 1
Text(13.37) = "float ;)"
EndPrmText
"#;

    let path = std::path::PathBuf::from(format!("{}", file!()));
    gsd_parser::parser::parse(&path, source);
}

#[test]
fn parse_invalid_prm_text2() {
    let source = r#"
#Profibus_DP
PrmText = 4.2
Text(1) = "float index ;)"
EndPrmText
"#;

    let path = std::path::PathBuf::from(format!("{}", file!()));
    gsd_parser::parser::parse(&path, source);
}
