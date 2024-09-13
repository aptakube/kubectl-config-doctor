use std::error::Error;

use colored::{ColoredString, Colorize, CustomColor};
use kube::config::KubeconfigError;

pub trait ColorizeExt {
    fn grey(&self) -> ColoredString;
    fn light_grey(&self) -> ColoredString;
}

impl ColorizeExt for str {
    fn grey(&self) -> ColoredString {
        self.custom_color(CustomColor::new(82, 82, 82))
    }
    fn light_grey(&self) -> ColoredString {
        self.custom_color(CustomColor::new(163, 163, 163))
    }
}

pub fn green_check() -> String {
    "✓".green().to_string()
}

pub fn red_cross() -> String {
    "✖".red().to_string()
}

pub fn print_title(title: &str) {
    println!("{}", title.bold());
}

pub fn print_error(err: String) {
    println!("{} {}", red_cross(), err.red());
}

pub fn expand_kubeerror(err: kube::Error) -> String {
    let mut error_msg = err.to_string();
    let mut source = err.source();
    while let Some(err_source) = source {
        error_msg.push_str(&format!(": {}", err_source));
        source = err_source.source();
    }

    return error_msg;
}

pub fn expand_kubeconfigerror(err: KubeconfigError) -> String {
    let mut error_msg = err.to_string();
    let mut source = err.source();
    while let Some(err_source) = source {
        error_msg.push_str(&format!(": {}", err_source));
        source = err_source.source();
    }

    return error_msg;
}
