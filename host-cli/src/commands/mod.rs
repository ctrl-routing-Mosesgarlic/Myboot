//! Host CLI subcommands (report §3.10.3). Each module = one command (SRP).
pub mod status;
pub mod bless;
pub mod config;
pub mod boot_order;
pub mod install;
pub mod uninstall;

pub type CmdResult = Result<(), Box<dyn std::error::Error>>;

pub fn print_help() {
    println!("myboot — host-side management for the MyBoot boot manager\n");
    println!("USAGE:");
    println!("  myboot install [--esp DIR] [--efi FILE]  install MyBoot + create its config");
    println!("  myboot uninstall [--esp DIR]              remove MyBoot (other OSes stay bootable)");
    println!("  myboot status              show Secure Boot state and boot order");
    println!("  myboot boot-order          print the firmware BootOrder");
    println!("  myboot bless               mark the current boot as good (confirm)");
    println!("  myboot bless --entry <id>  mark a specific entry as last-good");
    println!("  myboot config show         print the on-disk MyBoot config");
    println!("  myboot config path <file>  validate a config file");
    println!("  myboot help                show this help\n");
    println!("Most commands read/write UEFI variables via efivarfs and need root.");
}
