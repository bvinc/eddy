// Copyright 2018 The xi-editor Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::ffi::OsStr;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use url::Url;
// use xi_plugin_lib::{Cache, ChunkCache, CoreProxy, Error as PluginLibError, View};
// use xi_rope::rope::RopeDelta;

// use super::conversion_utils::*;
// use super::language_server_client::LanguageServerClient;
// use super::lsp_types::*;
// use super::parse_helper;
use super::parse_helper;
use super::result_queue::ResultQueue;
use super::types::Error;
use super::LanguageServerClient;
use log::*;

/// Start a new Language Server Process by spawning a process given the parameters
/// Returns a Arc to the Language Server Client which abstracts connection to the
/// server
pub fn start_new_server(
    command: String,
    arguments: Vec<String>,
    file_extensions: Vec<String>,
    language_id: &str,
    // core: CoreProxy,
    result_queue: ResultQueue,
) -> Result<Arc<Mutex<LanguageServerClient>>, String> {
    let mut process = Command::new(command)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Error Occurred");

    let writer = Box::new(BufWriter::new(process.stdin.take().unwrap()));

    let language_server_client = Arc::new(Mutex::new(LanguageServerClient::new(
        writer,
        // core,
        result_queue,
        language_id.to_owned(),
        file_extensions,
    )));

    {
        let ls_client = language_server_client.clone();
        let mut stdout = process.stdout;

        // Unwrap to indicate that we want thread to panic on failure
        std::thread::Builder::new()
            .name(format!("{}-lsp-stdout-Looper", language_id))
            .spawn(move || {
                let mut reader = Box::new(BufReader::new(stdout.take().unwrap()));
                loop {
                    match parse_helper::read_message(&mut reader) {
                        Ok(message_str) => {
                            let mut server_locked = ls_client.lock().unwrap();
                            server_locked.handle_message(message_str.as_ref());
                        }
                        Err(err) => error!("Error occurred {:?}", err),
                    };
                }
            })
            .unwrap();
    }

    Ok(language_server_client)
}
