extern crate docopt;
extern crate rustc_serialize;
extern crate package_manager;

use std::result;
use std::env;
use package_manager::manifest::{serialise_manifest, read_manifest};
use package_manager::error::Error;

pub const USAGE: &'static str = "Test page.

Usage:
    pm test [options]

Options:
    -h, --help     Display this message.
    --bdd          Use the Official BDD Style.
";

#[derive(Debug, RustcDecodable)]
pub struct Args {
    flag_bdd: bool,
}



pub fn execute(args: Args) -> result::Result<(), Error> {
    if args.flag_bdd {
        println!("As the test command, when I am called, then I am the test command.")
    } else {
        println!("This is the test command.")
    }
    println!(
        "Also, my working directory is {:?}\n",
        env::current_dir().unwrap().display()
    );

    println!("Here is the manifest file I found there:\n");
    let manifest = read_manifest()?;
    println!("{:?}", manifest);

    println!("\nHere it is re-serialised:\n");
    println!("{}", serialise_manifest(&manifest).unwrap());

    Ok(())
}
