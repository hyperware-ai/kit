mod handle_http;
mod handle_hyperware;

// Re-export the functions
pub use handle_http::handle_http_server_request;
pub use handle_hyperware::{
    handle_timer_message, handle_terminal_message, 
    handle_internal_message, handle_external_message
};

#[allow(unused_imports)]
use hyperware_process_lib::Address;

// Helper functions for address creation
pub fn make_http_address(our: &Address) -> Address {
    Address::from((our.node(), "http-server", "distro", "sys"))
}

pub fn make_timer_address(our: &Address) -> Address {
    Address::from((our.node(), "timer", "distro", "sys"))
}

pub fn make_terminal_address(our: &Address) -> Address {
    Address::from((our.node(), "terminal", "distro", "sys"))
}