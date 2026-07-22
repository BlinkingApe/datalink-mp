// Stub implementation for _Unwind_Resume
// This is needed because the Rust std library for Windows GNU
// references this symbol even when panic=abort is set.
// libgcc_eh provides other unwind symbols but not this one.

void _Unwind_Resume(void) {
    // Should never be called with panic=abort
    // If we get here, something is very wrong - just halt
    while(1) {}
}
