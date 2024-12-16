#[derive(macros::ConfigProperty, PartialEq, Debug)]
#[cfg_prop(name(TomlConfig), derive(Default))]
struct Config {
    value: i32,
    value2: String,

    #[cfg_prop(use_type(TomlNestedConfig), also_from(name = value1, mergeable))]
    value3: NestedConfig,

    #[cfg_prop(use_type(TomlNestedConfig), also_from(name = value1, mergeable))]
    value4: NestedConfig,
}

#[derive(macros::ConfigProperty, PartialEq, Debug)]
#[cfg_prop(name(TomlNestedConfig), derive(Clone, Default))]
struct NestedConfig {
    #[cfg_prop(also_from(name = value3))]
    value: f32,

    value1: bool,

    #[cfg_prop(also_from(name = value3))]
    value2: f32,
}

#[test]
fn simple_test() {
    let nested_config = TomlNestedConfig {
        value: None,
        value1: None,
        value2: Some(0.0),
        value3: Some(1.0),
    };

    assert_eq!(
        nested_config.unwrap_or_default(),
        NestedConfig {
            value: 1.0,
            value1: false,
            value2: 0.0
        }
    )
}

#[test]
fn use_defaults() {
    #[derive(macros::ConfigProperty, PartialEq, Debug)]
    #[cfg_prop(name(TomlSample), derive(Default))]
    struct Sample {
        #[cfg_prop(default)]
        value: i32,
        #[cfg_prop(default(true))]
        value2: bool,
        #[cfg_prop(default(path = Sample::default_string))]
        value3: String,
    }

    impl Sample {
        fn default_string() -> String {
            "Hell".to_string()
        }
    }

    let sample = TomlSample::default().unwrap_or_default();
    assert_eq!(
        sample,
        Sample {
            value: 0,
            value2: true,
            value3: "Hell".to_string()
        }
    )
}

#[test]
fn temporary_fields() {
    #[derive(macros::ConfigProperty, PartialEq, Debug)]
    #[cfg_prop(name(TomlSample), derive(Default))]
    struct Sample {
        #[cfg_prop(also_from(name = value5))]
        value: i32,
        #[cfg_prop(also_from(name = value5))]
        value1: i32,
        value2: f32,
    }

    let sample = TomlSample {
        value5: Some(30),
        ..Default::default()
    }
    .unwrap_or_default();

    assert_eq!(
        sample,
        Sample {
            value: 30,
            value1: 30,
            value2: 0.0
        }
    )
}

#[test]
fn complex_test() {
    let nested = TomlNestedConfig {
        value3: Some(1.0),

        ..Default::default()
    };

    let second_nested = TomlNestedConfig {
        value1: Some(true),
        ..Default::default()
    };

    let config = TomlConfig {
        value: Some(30),
        value1: Some(nested.clone()),
        value4: Some(second_nested),
        ..Default::default()
    }
    .unwrap_or_default();

    assert_eq!(
        config,
        Config {
            value: 30,
            value2: "".to_string(),
            value3: nested.unwrap_or_default(),
            value4: NestedConfig {
                value: 1.0,
                value1: true,
                value2: 1.0
            }
        }
    )
}
