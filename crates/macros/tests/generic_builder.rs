use shared::value::{TryFromValue, Value};

#[derive(macros::GenericBuilder, Eq, PartialEq, Debug)]
#[gbuilder(name(GBuilderTest))]
struct Test {
    #[gbuilder(aliases(field4))]
    field1: usize,
    #[gbuilder(aliases(field4), default)]
    field5: usize,
    field2: String,
    #[gbuilder(hidden, default)]
    field3: Option<u32>,
}

#[test]
fn build_struct() -> Result<(), Box<dyn std::error::Error>> {
    let mut gbuilder = GBuilderTest::new();

    gbuilder.set_value("field4", Value::UInt(3))?;
    gbuilder.set_value("field2", Value::String("hell".to_string()))?;

    let failure_assignment = gbuilder.set_value("field3", Value::UInt(5));
    assert!(failure_assignment.is_err());
    assert_eq!(
        failure_assignment.err().unwrap().to_string(),
        shared::error::ConversionError::UnknownField {
            field_name: "field3".to_string(),
            value: Value::UInt(5),
        }
        .to_string()
    );

    let result = gbuilder.try_build()?;

    assert_eq!(
        result,
        Test {
            field1: 3,
            field5: 3,
            field2: "hell".to_string(),
            field3: None
        }
    );
    Ok(())
}

#[derive(macros::GenericBuilder, Debug, Eq, PartialEq)]
#[gbuilder(name(GBuilderComplexStructure))]
struct ComplexStructure {
    field1: usize,
    field2: String,
    field3: InnerStructure,
}

#[derive(Debug, Eq, PartialEq, Clone)]
enum InnerStructure {
    First,
    Second,
}

impl TryFromValue for InnerStructure {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        match &*value {
            "first" => Ok(InnerStructure::First),
            "second" => Ok(InnerStructure::Second),
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "first or second",
                actual: value,
            }),
        }
    }
}

#[test]
fn build_complex_structure() -> Result<(), Box<dyn std::error::Error>> {
    let mut gbuilder = GBuilderComplexStructure::new();

    gbuilder
        .set_value("field1", Value::UInt(5))?
        .set_value("field2", Value::String("hell".to_string()))?
        .set_value("field3", Value::String("first".to_string()))?;

    let mut second_gbuilder = GBuilderComplexStructure::new();
    let inner_value = Value::Any(Box::new(InnerStructure::First));

    second_gbuilder
        .set_value("field1", Value::UInt(5))?
        .set_value("field2", Value::String("hell".to_string()))?
        .set_value("field3", inner_value)?;

    let first_struct = gbuilder.try_build()?;
    let second_struct = second_gbuilder.try_build()?;
    assert_eq!(first_struct, second_struct);

    assert_eq!(
        first_struct,
        ComplexStructure {
            field1: 5,
            field2: "hell".to_string(),
            field3: InnerStructure::First
        }
    );

    Ok(())
}

#[test]
#[should_panic(expected = "The field 'field1' should be set")]
fn empty_builder_should_panic() {
    GBuilderComplexStructure::new().try_build().unwrap();
}

#[test]
#[should_panic(expected = "The field 'field2' should be set")]
fn not_fulled_builder_should_panic() {
    let mut gbuilder = GBuilderComplexStructure::new();
    gbuilder.set_value("field1", Value::UInt(5)).unwrap();
    gbuilder.try_build().unwrap();
}
