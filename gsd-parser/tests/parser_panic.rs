#[test]
fn parse_invalid_number() {
    let source = r#"
#Profibus_DP
PrmText = 1
Text(13.37) = "float ;)"
EndPrmText
"#;

    let path = std::path::PathBuf::from(format!("{}", file!()));
    println!("{}", gsd_parser::parser::parse(&path, source).unwrap_err());
}

#[test]
fn parse_invalid_number2() {
    let source = r#"
#Profibus_DP
PrmText = 4.2
Text(0x1) = "float index ;)"
EndPrmText
"#;

    let path = std::path::PathBuf::from(format!("{}", file!()));
    println!("{}", gsd_parser::parser::parse(&path, source).unwrap_err());
}

#[test]
fn parse_number_overflow() {
    let source = r#"
#Profibus_DP
maxtsdr_9.6 = 4242424
"#;

    let path = std::path::PathBuf::from(format!("{}", file!()));
    println!("{}", gsd_parser::parser::parse(&path, source).unwrap_err());
}
