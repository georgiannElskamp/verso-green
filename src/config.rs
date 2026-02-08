// Fix for syntax error in src/config.rs:8
// Correctly call Response::network_error()

// src/config.rs
impl Config {
    pub fn handle_network_error() {
        Response::network_error();
    }
}
