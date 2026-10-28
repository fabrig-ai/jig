use super::*;

#[test]
fn parameter_conversion_names_its_count_ceiling_and_admits_equality() {
    let mut limits = limits();
    limits.tasks.parameters = 1;
    let parameters =
        Box::new([people::Parameter::Number { name: 1, value: 2 }, people::Parameter::Number { name: 2, value: 3 }]);
    let value = people::Spec { words: Box::new([]), parameters, inputs: Box::new([]) };
    assert_eq!(
        amendments::spec(value, &limits.tasks),
        Err(people::Refusal::configured(
            people::LimitKind::Spec,
            1,
            people::LimitKey::TasksParameters,
            people::LimitUnit::Items,
        ))
    );
    let value = people::Spec {
        words: Box::new([]),
        parameters: Box::new([people::Parameter::Number { name: 1, value: 2 }]),
        inputs: Box::new([]),
    };
    assert_eq!(amendments::spec(value, &limits.tasks).expect("at the ceiling").parameters.len(), 1);
}
