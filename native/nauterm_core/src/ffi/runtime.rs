#[no_mangle]
pub extern "C" fn nauterm_runtime_initialize() {
    crate::runtime::initialize();
}

#[no_mangle]
pub extern "C" fn nauterm_runtime_prepare_shutdown() {
    crate::runtime::prepare_shutdown();
}
