//! Actions module for handling AI model outputs.

mod handler;

pub use handler::{
    do_action, finish_action, parse_action, ActionHandler, ActionResult,
    ConfirmationCallback, TakeoverCallback,
};
