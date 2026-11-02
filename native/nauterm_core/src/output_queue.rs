use std::sync::{Arc, Mutex};

const MAX_DRAIN_OUTPUT_BYTES: usize = 16 * 1024;

pub fn append_output(output: &Arc<Mutex<Vec<u8>>>, bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    let Ok(mut output) = output.lock() else {
        return false;
    };

    output.extend_from_slice(bytes);
    true
}

pub fn drain_output_chunk(output: &Arc<Mutex<Vec<u8>>>) -> (Vec<u8>, bool) {
    let Ok(mut output) = output.lock() else {
        return (Vec::new(), false);
    };

    let drain_len = output.len().min(MAX_DRAIN_OUTPUT_BYTES);
    if drain_len == output.len() {
        return (std::mem::take(&mut *output), false);
    }

    let drained = output.drain(..drain_len).collect();
    (drained, true)
}

pub fn clear_output(output: &Arc<Mutex<Vec<u8>>>) {
    if let Ok(mut output) = output.lock() {
        output.clear();
        output.shrink_to_fit();
    }
}
