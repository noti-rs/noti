bitflags::bitflags! {
    /// DirtyFlags represent the pending updates for a widget.
    /// They prevent redundant calculations by marking exactly what needs
    /// synchronization or re-measurement in the current frame.
    #[derive(Clone, Copy)]
    pub(crate) struct DirtyFlags: u8 {
        const NEEDS_REBUILD       = 1;

        /// The widget's own geometric constraints or intrinsic sizes are invalid.
        /// Triggers a re-calculation of the cached size in the Arena.
        const NEEDS_MEASURE       = 1 << 1;

        /// One or more descendants are marked with NEEDS_MEASURE.
        /// Allows the system to skip clean branches during the measure pass.
        const CHILD_NEEDS_MEASURE = 1 << 2;
    }
}
