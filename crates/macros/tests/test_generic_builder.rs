enum Value {
    I32(i32),
    String(String),
    Any(Box<dyn std::any::Any>),
}

impl TryFrom<Value> for i32 {
    type Error = String;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::I32(val) = value {
            Ok(val)
        } else {
            Err("This is not i32 type".to_string())
        }
    }
}

impl TryFrom<Value> for String {
    type Error = String;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::String(val) = value {
            Ok(val)
        } else {
            Err("This is not String type".to_string())
        }
    }
}

#[derive(macros::GenericBuilder, Eq, PartialEq, Debug)]
#[gbuilder(name(GBuilderTest))]
struct Test {
    field1: i32,
    field2: String,
    #[gbuilder(hidden, default)]
    field3: Option<u32>,
}

#[test]
fn check_availability_of_fields() {
    let gbuilder = GBuilderTest::new();

    assert!(gbuilder.contains_field("field1"));
    assert!(gbuilder.contains_field("field2"));
    assert!(!gbuilder.contains_field("field3"));
    assert!(!gbuilder.contains_field("field4"));
}

#[test]
fn build_struct() -> Result<(), Box<dyn std::error::Error>> {
    let mut gbuilder = GBuilderTest::new();

    gbuilder.set_value("field1", Value::I32(3))?;
    gbuilder.set_value("field2", Value::String("hell".to_string()))?;

    let failure_assignment = gbuilder.set_value("field3", Value::I32(5));
    assert!(failure_assignment.is_err());
    assert_eq!(
        failure_assignment.err().unwrap().to_string(),
        "Unknown field"
    );

    let result = gbuilder.try_build()?;

    assert_eq!(
        result,
        Test {
            field1: 3,
            field2: "hell".to_string(),
            field3: None
        }
    );
    Ok(())
}

#[derive(macros::GenericBuilder, Debug, Eq, PartialEq)]
#[gbuilder(name(GBuilderComplexStructure))]
struct ComplexStructure {
    field1: i32,
    field2: String,
    field3: InnerStructure,
}

#[derive(Debug, Eq, PartialEq)]
enum InnerStructure {
    First,
    Second,
}

impl TryFrom<Value> for InnerStructure {
    type Error = String;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(str) => match &*str {
                "first" => Ok(InnerStructure::First),
                "second" => Ok(InnerStructure::Second),
                _ => Err("Unknown value: ".to_string() + &str),
            },
            Value::Any(boxed_object) => Ok(*boxed_object
                .downcast()
                .or(Err("Type of Any object is not InnerStructure.".to_string()))?),
            _ => Err("Cannot convert from value to InnerStructure".to_string()),
        }
    }
}

#[test]
fn build_complex_structure() -> Result<(), Box<dyn std::error::Error>> {
    let mut gbuilder = GBuilderComplexStructure::new();

    gbuilder
        .set_value("field1", Value::I32(5))?
        .set_value("field2", Value::String("hell".to_string()))?
        .set_value("field3", Value::String("first".to_string()))?;

    let mut second_gbuilder = GBuilderComplexStructure::new();
    let inner_value = Value::Any(Box::new(InnerStructure::First));

    second_gbuilder
        .set_value("field1", Value::I32(5))?
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
    gbuilder.set_value("field1", Value::I32(5)).unwrap();
    gbuilder.try_build().unwrap();
}
