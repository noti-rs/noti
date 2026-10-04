/// Describes the interaction input behavior of a widget from event.
///
/// It's effectively made for collecting input regions from widgets. Normally each widget accepts
/// inputs, giving a single rectangle covering a whole widget tree. But in some cases parent nodes
/// must not accept inputs and pass to children, that in result creates some rectangles covering
/// only children.
#[derive(Default, Debug, Clone, Copy)]
pub enum InputBehavior {
    /// A current widget accepts all inputs.
    #[default]
    Accepts,

    /// A current widget doesn't accepts all inputs, making a possibility to pass clicks through
    /// itself to an application at the back.
    ///
    /// For children, they're will be checked independently.
    PassesToChildren
}
