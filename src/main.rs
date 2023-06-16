use std::collections::HashSet;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::fs;
use std::net::SocketAddrV4;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use downloader::Downloader;
use ftp::FtpStream;

const SPLATOON_EU_TITLE_ID: &str = "10176A00";
const SPLATOON_USA_TITLE_ID: &str = "10176900";
const SPLATOON_JPN_TITLE_ID: &str = "10162B00";

const NOHASH_GAMBIT_URL: &str = "https://raw.githubusercontent.com/Splatoon-1-Database/s1eftp/main/Gambit.rpx";

fn get_choice() -> u32 {
    let input_text = get_string();

    let choice = input_text.trim().parse::<u32>().unwrap();

    choice
}

fn get_string() -> String {
    print!("> ");
    io::stdout().lock().flush().unwrap();

    let mut stdin = io::stdin().lock();

    let mut input_text = String::new();
    stdin
        .read_line(&mut input_text)
        .unwrap();

    input_text.trim().into()
}

fn wiiu_install_helper(title_ids: &Vec<String>, source_path: &Path, install_path: &Path, ftp_stream: &mut FtpStream) {
    for title_id in title_ids {
        let source_path_paths = walkdir::WalkDir::new(&source_path);
        for path in source_path_paths {
            let path = path.unwrap();
            if path.path().is_dir() {
                continue
            }
            let path_str = path.path().to_str().unwrap();
            let mut path_str = path_str.replace("\\", "/");

            if source_path.ends_with("/") {
                path_str.pop();
            }

            let path_trimmed = &path_str[
                source_path.to_str().unwrap().len() + 1..];

            println!("Writing \"{path_trimmed}\"...", );

            let mut file = fs::File::open(path.path()).unwrap();

            ftp_stream.put(&format!(
                "{}/{}/{}",
                install_path.to_str().unwrap(),
                title_id,
                path_trimmed,
            ), &mut file).unwrap();
        }
    }
}

fn wiiu_setup() {
    println!("Please make sure that system level access via FTP is enabled.");
    println!("* If you are on Aroma homebrew environment, to enable system level access via FTP you have to do the following:");
    println!("1. Go to the Wii U menu.");
    println!("2. Press the following keys at the same time: L + DPad-Down + Select.");
    println!("3. Go to the \"FTPiiU\" section (if you don't have FTPiiU installed, download it from here: https://github.com/wiiu-env/ftpiiu_plugin/releases).");
    println!("4. Go to the \"Settings\" section.");
    println!("Set \"Allow access to system files\" to true.");

    println!("Press any key once you have completed these steps.");
    get_string();

    println!("Please enter the IP address of your Wii U to connect via FTP.");
    println!("* If you are on Aroma homebrew environment, to find your IP address you have to do the following:");
    println!("1. Go to the Wii U menu.");
    println!("2. Press the following keys at the same time: L + DPad-Down + Select.");
    println!("3. Go to the \"FTPiiU\" section (if you don't have FTPiiU installed, download it from here: https://github.com/wiiu-env/ftpiiu_plugin/releases).");
    println!("4. Your IP address should be displayed. Usually, it starts with \"192.168\".");

    let Ok(socket_address) = SocketAddrV4::from_str(&format!("{}:21", &get_string())) else {
        println!("Invalid IP address provided. Make sure that the port is NOT included in the input.");
        return
    };

    let mut ftp_stream = FtpStream::connect(socket_address).unwrap();
    ftp_stream.login("", "").unwrap();

    let splatoon_title_ids = HashSet::from([SPLATOON_EU_TITLE_ID, SPLATOON_USA_TITLE_ID, SPLATOON_JPN_TITLE_ID]);

    let mlc_path = Path::new("storage_mlc/usr/title/0005000e");
    let usb_path = Path::new("storage_usb/usr/title/0005000e");

    let mut installed_title_ids_mlc = ftp_stream.nlst(Some(mlc_path.to_str().unwrap())).unwrap();
    installed_title_ids_mlc.retain(|title_id| {
        splatoon_title_ids.contains(title_id.to_uppercase().as_str())
    });

    let mut installed_title_ids_usb = ftp_stream.nlst(Some(usb_path.to_str().unwrap())).unwrap();
    installed_title_ids_usb.retain(|title_id| {
        splatoon_title_ids.contains(title_id.to_uppercase().as_str())
    });

    if installed_title_ids_mlc.is_empty() && installed_title_ids_usb.is_empty() {
        println!("Splatoon could not be detected.");
    }

    println!("Splatoon detected successfully. Please enter the FULL path to the extracted mod directory.");
    println!("* The developer is not responsible for any damages causes by the files provided on the path.");

    let splatoon_mod_path = get_string();
    let splatoon_mod_path = Path::new(&splatoon_mod_path);

    if !installed_title_ids_mlc.is_empty() {
        wiiu_install_helper(&installed_title_ids_mlc, &splatoon_mod_path, mlc_path, &mut ftp_stream);
    }

    if !installed_title_ids_usb.is_empty() {
        wiiu_install_helper(&installed_title_ids_usb, &splatoon_mod_path, usb_path, &mut ftp_stream);
    }

    println!("Would you like to install No-Hash to prevent disconnects caused by the mod? (1 for yes, 2 for no)");
    println!("* Using mods in matches on Nintendo Network without some form of No-Hash will get you instantly banned.");

    let choice = get_choice();

    match choice {
        1 => {},
        2 => return,
        _ => {
            println!("Invalid choice");
            return
        },
    }

    let nohash_download_path = download_nohash();
    let nohash_download_path = nohash_download_path.as_path();

    if !installed_title_ids_mlc.is_empty() {
        wiiu_install_helper(&installed_title_ids_mlc, nohash_download_path, mlc_path, &mut ftp_stream);
    }

    if !installed_title_ids_usb.is_empty() {
        wiiu_install_helper(&installed_title_ids_usb, nohash_download_path, usb_path, &mut ftp_stream);
    }

    ftp_stream.quit().unwrap();
}

fn download_nohash() -> PathBuf {
    println!("Downloading No-Hash...");

    let mut download_path = std::env::temp_dir();

    download_path.push("nohash");

    let download_root = download_path.clone();

    download_path.push("code");

    fs::create_dir_all(&download_path).unwrap();

    let mut downloader = Downloader::builder()
        .download_folder(download_path.as_path())
        .parallel_requests(1)
        .build()
        .unwrap();

    let nohash_download = downloader::Download::new(NOHASH_GAMBIT_URL);

    downloader.download(&[nohash_download]).unwrap();

    download_root
}

fn cemu_setup() {
    println!("Cemu support not yet implemented.");
}

fn main() {
    println!("================================");
    println!("splatoon1enjoyer's Splatoon Mods Installer");
    println!("================================");
    println!("Are you on Wii U or Cemu? (1 for Wii U, 2 for Cemu)");

    let choice = get_choice();

    match choice {
        1 => wiiu_setup(),
        2 => cemu_setup(),
        _ => println!("Invalid choice"),
    }
}