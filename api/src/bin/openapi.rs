//! Writes the `OpenAPI` document to stdout.
//!
//! `pnpm openapi:generate` sends it to `api/openapi.json`, and the web client is
//! generated from that. Needs no database and no configuration, so continuous
//! integration can run it on its own.

use anyhow::Context as _;

use api::presentation::openapi;

fn main() -> anyhow::Result<()> {
    let document = openapi::to_pretty_json().context("could not serialise the OpenAPI document")?;
    println!("{document}");
    Ok(())
}
