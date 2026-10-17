//! Application paths as the driver keeps them: `C:\Games\x.exe` <-> `\Device\HarddiskVolume3\Games\x.exe`
//! (like `ref/HidHide/HidHideCLI/src/Volume.cpp`). The file does not have to exist.

use std::path::{Path, PathBuf};

/// A mounted volume: its mount points (`C:\`, `D:\mnt\data\`) and NT device (`\Device\HarddiskVolume3`).
#[derive(Debug, Clone)]
pub struct Volume {
    pub mount_points: Vec<String>,
    pub device: String,
}

fn starts_with_ignore_case(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len()
        && s.is_char_boundary(prefix.len())
        && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// `\Device\HarddiskVolume3\...` for `path`, using the deepest mount point that contains it.
pub fn to_image_name_in(volumes: &[Volume], path: &Path) -> Option<String> {
    let path = path.to_str()?;
    let (mount, device) = volumes
        .iter()
        .flat_map(|v| v.mount_points.iter().map(move |m| (m, &v.device)))
        .filter(|(m, _)| starts_with_ignore_case(path, m))
        .max_by_key(|(m, _)| m.len())?;
    Some(format!("{device}\\{}", &path[mount.len()..]))
}

/// The path with a drive letter (or folder mount point) for a driver image name.
pub fn from_image_name_in(volumes: &[Volume], image: &str) -> Option<PathBuf> {
    volumes.iter().find_map(|v| {
        let prefix = format!("{}\\", v.device);
        if !starts_with_ignore_case(image, &prefix) {
            return None;
        }
        let mount = v.mount_points.first()?;
        Some(PathBuf::from(format!("{mount}{}", &image[prefix.len()..])))
    })
}

/// All mounted volumes with at least one mount point.
#[cfg(windows)]
pub fn volumes() -> Vec<Volume> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        FindFirstVolumeW, FindNextVolumeW, FindVolumeClose, GetVolumePathNamesForVolumeNameW,
        QueryDosDeviceW,
    };

    let wide = |s: &[u16]| {
        let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        String::from_utf16_lossy(&s[..end])
    };
    let mut result = Vec::new();
    let mut name = vec![0u16; 1024];
    // SAFETY: the buffers are large enough and live across each call.
    let Ok(find) = (unsafe { FindFirstVolumeW(&mut name) }) else {
        return result;
    };
    loop {
        // `\\?\Volume{guid}\`
        let volume = wide(&name);
        let z: Vec<u16> = volume.encode_utf16().chain([0]).collect();
        let mut paths = vec![0u16; 4096];
        let mut len = 0u32;
        let mounts = unsafe {
            GetVolumePathNamesForVolumeNameW(PCWSTR(z.as_ptr()), Some(&mut paths), &mut len)
        }
        .map(|()| crate::driver::from_multi_sz(&paths))
        .unwrap_or_default();
        // QueryDosDeviceW wants `Volume{guid}` without the `\\?\` prefix and trailing `\`.
        let short: Vec<u16> = volume
            .trim_start_matches(r"\\?\")
            .trim_end_matches('\\')
            .encode_utf16()
            .chain([0])
            .collect();
        let mut target = vec![0u16; 1024];
        let n = unsafe { QueryDosDeviceW(PCWSTR(short.as_ptr()), Some(&mut target)) };
        if n > 0 && !mounts.is_empty() {
            result.push(Volume {
                mount_points: mounts,
                device: wide(&target),
            });
        }
        name.fill(0);
        if unsafe { FindNextVolumeW(find, &mut name) }.is_err() {
            break;
        }
    }
    unsafe {
        let _ = FindVolumeClose(find);
    }
    result
}

#[cfg(not(windows))]
pub fn volumes() -> Vec<Volume> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<Volume> {
        vec![
            Volume {
                mount_points: vec![r"C:\".into()],
                device: r"\Device\HarddiskVolume3".into(),
            },
            Volume {
                mount_points: vec![r"C:\mnt\data\".into()],
                device: r"\Device\HarddiskVolume10".into(),
            },
            Volume {
                mount_points: vec![r"D:\".into()],
                device: r"\Device\HarddiskVolume1".into(),
            },
        ]
    }

    #[test]
    fn converts_both_ways() {
        let t = table();
        let exe = Path::new(r"C:\Игры\Star Citizen\game.exe");
        let image = to_image_name_in(&t, exe).unwrap();
        assert_eq!(image, r"\Device\HarddiskVolume3\Игры\Star Citizen\game.exe");
        assert_eq!(from_image_name_in(&t, &image).unwrap(), exe);
        // The deepest mount point wins; Volume1 is not a prefix of Volume10.
        let nested = Path::new(r"c:\mnt\data\x.exe");
        let image = to_image_name_in(&t, nested).unwrap();
        assert_eq!(image, r"\Device\HarddiskVolume10\x.exe");
        assert_eq!(
            from_image_name_in(&t, &image).unwrap(),
            Path::new(r"C:\mnt\data\x.exe")
        );
        assert_eq!(
            from_image_name_in(&t, r"\Device\HarddiskVolume1\a.exe").unwrap(),
            Path::new(r"D:\a.exe")
        );
    }

    #[test]
    fn unknown_volume_is_none() {
        let t = table();
        assert!(to_image_name_in(&t, Path::new(r"Z:\x.exe")).is_none());
        assert!(from_image_name_in(&t, r"\Device\HarddiskVolume7\x.exe").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn own_exe_round_trip() {
        let exe = std::env::current_exe().unwrap();
        let v = volumes();
        let image = to_image_name_in(&v, &exe).expect("exe on a mounted volume");
        assert!(image.starts_with(r"\Device\"), "{image}");
        let back = from_image_name_in(&v, &image).unwrap();
        assert!(crate::same_path(&back, &exe), "{back:?} != {exe:?}");
    }
}
