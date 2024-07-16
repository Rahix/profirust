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
fn parse_invalid_number_list() {
    let source = r#"
#Profibus_DP
Ext_User_Prm_Data_Const(0) = 40, 40, 40.2, 42
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

#[test]
fn parse_unknown_text_ref() {
    let source = r#"
#Profibus_DP
ExtUserPrmData=1 "Test Data"
Bit(0) 0 0-1
Prm_Text_Ref=1337
EndExtUserPrmData
"#;

    let path = std::path::PathBuf::from(format!("{}", file!()));
    println!("{}", gsd_parser::parser::parse(&path, source).unwrap_err());
}
