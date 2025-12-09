#[test]
fn slot_missing_allowed_module() {
    let source = r#"
#Profibus_DP
Module = "Module 1" 0x00
1
Ext_Module_Prm_Data_Len = 0
EndModule
SlotDefinition
Slot(1) = "Process Data Interface" 1 1-3
EndSlotDefinition
"#;

    let path = std::path::PathBuf::from(format!("{}", file!()));
    let (res, warnings) = gsd_parser::parser::parse_with_warnings(&path, source);
    res.unwrap();
    for warning in warnings.iter() {
        eprintln!("{}", warning);
    }
    assert!(warnings.len() == 2);
}

#[test]
fn slot_default_module_not_allowed() {
    let source = r#"
#Profibus_DP
Module = "Module 1" 0x00
1
Ext_Module_Prm_Data_Len = 0
EndModule
Module = "Module 2" 0x00
2
Ext_Module_Prm_Data_Len = 0
EndModule
Module = "Module 3" 0x00
3
Ext_Module_Prm_Data_Len = 0
EndModule
SlotDefinition
Slot(1) = "Process Data Interface" 1 2-3
EndSlotDefinition
"#;

    let path = std::path::PathBuf::from(format!("{}", file!()));
    let (res, warnings) = gsd_parser::parser::parse_with_warnings(&path, source);
    res.unwrap();
    for warning in warnings.iter() {
        eprintln!("{}", warning);
    }
    assert!(warnings.len() == 1);
}
