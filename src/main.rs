use downloader::Downloader;
use ftp::FtpStream;
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::net::SocketAddrV4;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const SPLATOON_EUR_TITLE_ID: &str = "10176A00";
const SPLATOON_USA_TITLE_ID: &str = "10176900";
const SPLATOON_JPN_TITLE_ID: &str = "10162B00";

const NOHASH_GAMBIT_URL: &str =
    "https://raw.githubusercontent.com/Splatoon-1-Database/s1eftp/main/Gambit.rpx";

trait PathBufExt {
    fn offset(&self, offset: usize) -> PathBuf;
}

impl PathBufExt for PathBuf {
    fn offset(&self, offset: usize) -> PathBuf {
        let mut path_iter = self.components();

        for _ in 0..offset {
            path_iter.next();
        }

        path_iter.collect::<PathBuf>()
    }
}

enum InstallStorage {
    MLC,
    USB,
}

impl InstallStorage {
    fn path(&self) -> PathBuf {
        let path = match self {
            Self::MLC => "storage_mlc\\usr\\title\\0005000e",
            Self::USB => "storage_usb\\usr\\title\\0005000e",
        };
        PathBuf::from_str(path).unwrap()
    }
}

fn detect_region(title_id: &str) -> Option<String> {
    let region = match title_id.to_uppercase().as_str() {
        "10176A00" => "EUR",
        "10176900" => "USA",
        "10162B00" => "JPN",
        _ => return None,
    };
    Some(region.into())
}

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
    stdin.read_line(&mut input_text).unwrap();

    input_text.trim().into()
}

fn install_files(
    title_id: &str,
    mod_path: &Path,
    storage_device: &InstallStorage,
    ftp_stream: &mut FtpStream,
) {
    println!("Installing files...");

    let offset = mod_path.components().count();

    let install_root = storage_device.path().join(title_id);

    for mod_path_entry in walkdir::WalkDir::new(&mod_path) {
        let mod_path_entry = mod_path_entry.unwrap();
        let mod_path_entry_buf = mod_path_entry
            .path()
            .components()
            .into_iter()
            .collect::<PathBuf>();
        let mod_path_entry_buf_cut = mod_path_entry_buf.offset(offset);

        let install_path = install_root.join(&mod_path_entry_buf_cut);

        if install_path.extension().is_none() {
            continue;
        }

        println!("Writing \"{}\"...", mod_path_entry_buf_cut.display());

        let mod_path = mod_path
            .join(&mod_path_entry_buf_cut)
            .iter()
            .collect::<PathBuf>();

        let mut file = fs::File::open(mod_path).unwrap();

        ftp_stream
            .put(
                &install_path.to_str().unwrap().replace("\\", "/"),
                &mut file,
            )
            .unwrap();
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

    let splatoon_title_ids = HashSet::from([
        SPLATOON_EUR_TITLE_ID,
        SPLATOON_USA_TITLE_ID,
        SPLATOON_JPN_TITLE_ID,
    ]);

    let mlc_path = Path::new("storage_mlc/usr/title/0005000e");
    let usb_path = Path::new("storage_usb/usr/title/0005000e");

    let mut installed_title_ids_mlc = ftp_stream.nlst(Some(mlc_path.to_str().unwrap())).unwrap();
    installed_title_ids_mlc
        .retain(|title_id| splatoon_title_ids.contains(title_id.to_uppercase().as_str()));

    let mut installed_title_ids_usb = ftp_stream.nlst(Some(usb_path.to_str().unwrap())).unwrap();
    installed_title_ids_usb
        .retain(|title_id| splatoon_title_ids.contains(title_id.to_uppercase().as_str()));

    if installed_title_ids_mlc.is_empty() && installed_title_ids_usb.is_empty() {
        println!("Splatoon could not be detected.");
        return;
    }

    let mut installed_splatoon_titles: Vec<(InstallStorage, String)> = vec![];

    for title_id in &installed_title_ids_mlc {
        if detect_region(&title_id).is_none() {
            continue;
        }
        installed_splatoon_titles.push((InstallStorage::MLC, title_id.clone()));
    }

    for title_id in &installed_title_ids_usb {
        if detect_region(&title_id).is_none() {
            continue;
        }
        installed_splatoon_titles.push((InstallStorage::USB, title_id.clone()));
    }

    if installed_splatoon_titles.is_empty() {
        println!("Splatoon could not be detected.");
        return;
    }

    println!("Which Splatoon version would you like to install the mod to? (enter the corresponding number)");

    for (i, title) in installed_splatoon_titles.iter().enumerate() {
        let display_storage = match title.0 {
            InstallStorage::MLC => "Console's Storage",
            InstallStorage::USB => "USB Storage",
        };
        let region = detect_region(&title.1).unwrap();
        println!("{i}. \"{region}\" Splatoon ({display_storage})",);
    }

    let choice = get_choice();

    let target_title = &installed_splatoon_titles[choice as usize];

    println!("Please enter the FULL path to the extracted mod directory.");
    println!("* The developer is not responsible for any damages causes by the files provided on the path.");

    let splatoon_mod_path = get_string();
    let splatoon_mod_path = Path::new(&splatoon_mod_path);

    println!("Would you like to do a backup of the files of the selected Splatoon title before installing the mod? (1 for yes, 2 for no)");

    let choice = get_choice();
    if choice == 1 {
        println!("Please enter the FULL path to the directory that should be used for the backup to your PC.");
        let backup_path = get_string();
        let backup_path = Path::new(&backup_path);
        backup_files(
            &target_title.1,
            splatoon_mod_path,
            &target_title.0,
            backup_path,
            &mut ftp_stream,
        );
    }

    install_files(
        &target_title.1,
        splatoon_mod_path,
        &target_title.0,
        &mut ftp_stream,
    );

    println!("Would you like to install No-Hash to prevent disconnects caused by the mod? (1 for yes, 2 for no)");
    println!("* Using mods in matches on Nintendo Network without some form of No-Hash will get you instantly banned.");

    let choice = get_choice();

    match choice {
        1 => {}
        _ => return,
    }

    let nohash_download_path = download_nohash();
    let nohash_download_path = nohash_download_path.as_path();

    install_files(
        &target_title.1,
        nohash_download_path,
        &target_title.0,
        &mut ftp_stream,
    );

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

fn backup_files(
    title_id: &str,
    mod_path: &Path,
    storage_device: &InstallStorage,
    backup_path: &Path,
    ftp_stream: &mut FtpStream,
) {
    println!("Backing up files...");

    let game_region = detect_region(title_id).unwrap();

    let offset = mod_path.components().count();

    let source_root = storage_device.path().join(title_id);

    for mod_path_entry in walkdir::WalkDir::new(&mod_path) {
        let mod_path_entry = mod_path_entry.unwrap();
        let mod_path_entry_buf = mod_path_entry
            .path()
            .components()
            .into_iter()
            .collect::<PathBuf>();
        let mod_path_entry_buf_cut = mod_path_entry_buf.offset(offset);

        let source_path = source_root.join(&mod_path_entry_buf_cut);

        if source_path.extension().is_none() {
            continue;
        }

        let Ok(cursor) = ftp_stream.simple_retr(&source_path.to_str().unwrap().replace("\\", "/")) else {
                println!("Backing up file \"{game_region}\" from Splatoon failed. Would you like to continue backing up? (1 for yes, 2 for no)");
                let choice = get_choice();
                match choice {
                    1 => continue,
                    _ => break,
                };
            };

        let backup_path = backup_path
            .join(&mod_path_entry_buf_cut)
            .iter()
            .collect::<PathBuf>();

        let mut backup_path_temp = backup_path.clone();
        backup_path_temp.pop();

        fs::create_dir_all(backup_path_temp).unwrap();

        println!("Writing \"{}\"...", mod_path_entry_buf_cut.display());

        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .append(false)
            .open(backup_path)
            .unwrap();

        file.write_all(cursor.into_inner().as_slice()).unwrap();
    }
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
        _ => cemu_setup(),
    }
}
