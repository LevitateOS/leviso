// Rocky Linux ISO download recipe
// Downloads the minimal ISO if not already present

let name = "rocky-iso";
let version = "9.5";

// Rocky 9.5 minimal ISO
let iso_url = "https://download.rockylinux.org/pub/rocky/9/isos/x86_64/Rocky-9.5-x86_64-minimal.iso";
let iso_path = "vendor/images/Rocky-9.5-x86_64-minimal.iso";

/// Check if ISO already exists
fn is_installed() {
    exists(iso_path)
}

/// Download the ISO
fn acquire() {
    if !dir_exists("vendor/images") {
        mkdir("vendor/images");
    }

    print(`     downloading Rocky ${version} minimal ISO (~2GB)...`);
    download(iso_url);

    // Move from BUILD_DIR to final location
    let downloaded = `${BUILD_DIR}/Rocky-9.5-x86_64-minimal.iso`;
    mv(downloaded, iso_path);
}

/// Nothing to install - ISO is a source, not a package
fn install() {
    print(`     ISO available at: ${iso_path}`);
}
