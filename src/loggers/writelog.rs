// Copyright 2016 Victor Brekenfeld
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! Module providing the FileLogger Implementation

use super::logging::try_log;
use crate::{Config, SharedLogger};
use log::{set_boxed_logger, set_max_level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::io::Write;
use std::sync::Mutex;

/// The WriteLogger struct. Provides a Logger implementation for structs implementing `Write`, e.g. File
pub struct WriteLogger<W: Write + Send + 'static> {
    level: LevelFilter,
    config: Config,
    writable: Mutex<W>,
    modules: Vec<String>,
}

impl<W: Write + Send + 'static> WriteLogger<W> {
    /// init function. Globally initializes the WriteLogger as the one and only used log facility.
    ///
    /// Takes the desired `Level`, `Config` and `Write` struct as arguments. They cannot be changed later on.
    /// Fails if another Logger was already initialized.
    ///
    /// # Examples
    /// ```
    /// # extern crate simplelog;
    /// # use simplelog::*;
    /// # use std::fs::File;
    /// # fn main() {
    /// let _ = WriteLogger::init(LevelFilter::Info, Config::default(), File::create("my_rust_bin.log").unwrap());
    /// # }
    /// ```
    pub fn init(
        log_level: LevelFilter,
        config: Config,
        writable: W,
        modules: Vec<String>,
    ) -> Result<(), SetLoggerError> {
        set_max_level(log_level.clone());
        set_boxed_logger(WriteLogger::new(log_level, config, writable, modules))
    }

    /// allows to create a new logger, that can be independently used, no matter what is globally set.
    ///
    /// no macros are provided for this case and you probably
    /// dont want to use this function, but `init()`, if you dont want to build a `CombinedLogger`.
    ///
    /// Takes the desired `Level`, `Config` and `Write` struct as arguments. They cannot be changed later on.
    ///
    /// # Examples
    /// ```
    /// # extern crate simplelog;
    /// # use simplelog::*;
    /// # use std::fs::File;
    /// # fn main() {
    /// let file_logger = WriteLogger::new(LevelFilter::Info, Config::default(), File::create("my_rust_bin.log").unwrap());
    /// # }
    /// ```
    pub fn new(
        log_level: LevelFilter,
        config: Config,
        writable: W,
        modules: Vec<String>,
    ) -> Box<WriteLogger<W>> {
        Box::new(WriteLogger {
            level: log_level,
            config: config,
            writable: Mutex::new(writable),
            modules,
        })
    }

    fn includes_module(&self, module_path: &str) -> bool {
        // If modules is empty, include all module paths
        if self.modules.is_empty() {
            return true;
        }
        // if a prefix of module_path is in `self.modules`, it must
        // be located at the first location before
        // where module_path would be.
        match self
            .modules
            .binary_search_by(|module| module.as_str().cmp(&module_path))
        {
            Ok(_) => {
                // Found exact module: return true
                true
            }
            Err(0) => {
                // if there's no item which would be located before module_path, no prefix is there
                false
            }
            Err(i) => is_submodule(&self.modules[i - 1], module_path),
        }
    }
}

impl<W: Write + Send + 'static> Log for WriteLogger<W> {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level && self.includes_module(metadata.target())
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let mut write_lock = self.writable.lock().unwrap();
            let _ = try_log(&self.config, record, &mut *write_lock);
        }
    }

    fn flush(&self) {
        let _ = self.writable.lock().unwrap().flush();
    }
}

impl<W: Write + Send + 'static> SharedLogger for WriteLogger<W> {
    fn level(&self) -> LevelFilter {
        self.level
    }

    fn config(&self) -> Option<&Config> {
        Some(&self.config)
    }

    fn as_log(self: Box<Self>) -> Box<dyn Log> {
        Box::new(*self)
    }
}

fn is_submodule(parent: &str, possible_child: &str) -> bool {
    // Treat as bytes, because we'll be doing slicing, and we only care about ':' chars
    let parent = parent.as_bytes();
    let possible_child = possible_child.as_bytes();

    // a longer module path cannot be a parent of a shorter module path
    if parent.len() > possible_child.len() {
        return false;
    }

    // If the path up to the parent isn't the same as the child,
    if parent != &possible_child[..parent.len()] {
        return false;
    }

    // Either the path is exactly the same, or the sub module should have a "::" after
    // the length of the parent path. This prevents things like 'a::bad' being considered
    // a submodule of 'a::b'
    parent.len() == possible_child.len()
        || possible_child.get(parent.len()..parent.len() + 2) == Some(b"::")
}
